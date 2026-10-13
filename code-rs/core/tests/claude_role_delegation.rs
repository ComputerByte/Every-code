#![cfg(unix)]

mod common;

use code_core::config::{Config, ConfigOverrides, ConfigToml};
use code_core::config_types::{AgentConfig, ReasoningEffort};
use code_core::protocol::{EventMsg, InputItem, Op};
use code_core::{CodexAuth, ConversationManager};
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::time::{timeout, Duration};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn tool_response(call: usize, arguments: Value) -> ResponseTemplate {
    let item = json!({"type":"response.output_item.done","item":{
        "type":"function_call","id":format!("item-{call}"),"call_id":format!("call-{call}"),
        "name":"agent","arguments":arguments.to_string()}});
    let done = json!({"type":"response.completed","response":{"id":format!("resp-{call}")}});
    ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
        .set_body_string(format!("data: {item}\n\ndata: {done}\n\n"))
}

fn tool_outputs(request: &Value) -> Vec<Value> {
    request["input"].as_array().expect("input").iter().filter_map(|item| {
        if item["type"] == "function_call_output" {
            item["output"].as_str().and_then(|output| serde_json::from_str(output).ok())
        } else { None }
    }).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial_test::serial]
async fn gpt_6_1_routes_claude_roles_waits_and_collects_success_and_failure() {
    let home = TempDir::new().expect("code home");
    let project = TempDir::new().expect("project");
    let mut agents = Vec::new();
    let cases = [
        ("review", "claude-opus-5-5", false),
        ("investigation", "claude-sonnet-5-5", false),
        ("architecture", "claude-haiku-5-5", false),
        ("failure", "claude-haiku-5-5", true),
    ];
    for (role, model, fail) in cases {
        let directory = project.path().join(role);
        std::fs::create_dir(&directory).expect("executable directory");
        let executable = directory.join("claude");
        let script = if fail {
            "#!/bin/sh\nprintf 'mock Claude failure' >&2\nexit 7\n"
        } else {
            "#!/bin/sh\nprintf 'ARGV_CAPTURE'\nfor arg in \"$@\"; do printf '|%s' \"$arg\"; done\n"
        };
        std::fs::write(&executable, script).expect("script");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).expect("permissions");
        agents.push(AgentConfig {
            name: role.to_string(), command: executable.display().to_string(),
            args: vec!["--model".to_string(), model.to_string()],
            read_only: true, enabled: true, description: None, env: None,
            args_read_only: Some(vec!["--model".to_string(), model.to_string(),
                "--permission-mode".to_string(), "default".to_string()]),
            args_write: None, instructions: None,
        });
    }
    let server = MockServer::start().await;
    let recorded = Arc::new(Mutex::new(Vec::<Value>::new()));
    let requests = Arc::clone(&recorded);
    Mock::given(method("POST")).respond_with(move |request: &Request| {
        let body: Value = serde_json::from_slice(&request.body).expect("request JSON");
        assert_eq!(body["model"], "gpt-6.1-sol");
        let mut requests = requests.lock().expect("requests");
        let step = requests.len();
        requests.push(body.clone());
        if step == 0 {
            return tool_response(step, json!({"action":"create","create":{
                "task":"Inspect the fixture and report the selected model",
                "models":["review","investigation","architecture","failure"],"read_only":true}}));
        }
        let outputs = tool_outputs(&body);
        let created = outputs.iter().find(|output| output["status"] == "started").expect("create result");
        let batch = created["batch_id"].as_str().expect("batch ID");
        let ids = created["agent_ids"].as_array().expect("agent IDs");
        assert_eq!(ids.len(), 4);
        if step == 1 {
            return tool_response(step, json!({"action":"wait","wait":{
                "batch_id":batch,"return_all":true,"timeout_seconds":5}}));
        }
        if step < 6 {
            return tool_response(step, json!({"action":"result","result":{
                "batch_id":batch,"agent_id":ids[step - 2]}}));
        }
        assert_eq!(step, 6, "unexpected extra coordinator requests");
        let item = json!({"type":"response.output_item.done","item":{
            "type":"message","id":"final","role":"assistant",
            "content":[{"type":"output_text","text":"Collected three successes and one failure."}]}});
        let done = json!({"type":"response.completed","response":{"id":"final"}});
        ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
            .set_body_string(format!("data: {item}\n\ndata: {done}\n\n"))
    }).expect(7).mount(&server).await;
    let base = ConfigToml { agents, ..ConfigToml::default() };
    let mut config = Config::load_from_base_config_with_overrides(base,
        ConfigOverrides::default(), home.path().to_path_buf()).expect("parsed and merged config");
    config.cwd = project.path().to_path_buf();
    config.model = "gpt-6.1-sol".to_string();
    config.model_family = code_core::model_family::find_family_for_model(&config.model).expect("family");
    config.model_reasoning_effort = ReasoningEffort::High;
    config.model_provider.base_url = Some(server.uri());
    config.model_provider.supports_websockets = false;
    config.model_provider.stream_max_retries = Some(0);
    let conversation = ConversationManager::with_auth(CodexAuth::from_api_key("mock-key"))
        .new_conversation(config).await.expect("conversation").conversation;
    conversation.submit(Op::UserInput {
        items: vec![InputItem::Text { text: "Delegate to the configured roles.".to_string() }],
        final_output_json_schema: None,
    }).await.expect("submit");
    timeout(Duration::from_secs(15), common::wait_for_event(&conversation,
        |event| matches!(event, EventMsg::TaskComplete(_)))).await.expect("bounded delegation");
    let requests = recorded.lock().expect("requests");
    assert_eq!(requests.len(), 7);
    let outputs = tool_outputs(requests.last().expect("last request"));
    let results: Vec<_> = outputs.iter().filter(|output| output.get("output_preview").is_some()).collect();
    assert_eq!(results.len(), 3);
    for (_, model, fail) in cases {
        if fail { continue; }
        let result = results.iter().find(|result| result["output_preview"].as_str()
            .is_some_and(|output| output.contains(model))).expect("exact model result");
        let output = result["output_preview"].as_str().expect("output");
        assert!(output.contains("--allowedTools|"));
        assert!(output.contains("--permission-mode|default"));
        assert!(output.contains("--effort|high"));
        assert!(!output.contains("--dangerously-skip-permissions"));
        assert_eq!(result["status"], "completed");
    }
    assert!(outputs.iter().any(|output| output["status"] == "failed"
        && output["error_preview"].as_str().is_some_and(|error| error.contains("mock Claude failure"))));
    drop(requests);
    conversation.submit(Op::Shutdown).await.expect("shutdown");
}
