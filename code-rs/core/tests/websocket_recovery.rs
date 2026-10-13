mod common;

use code_core::built_in_model_providers;
use code_core::protocol::{EventMsg, InputItem, Op};
use code_core::{CodexAuth, ConversationManager};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::Message;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn completed() -> Value {
    json!({"type":"response.completed","response":{"id":"recovery","usage":{
        "input_tokens":1,"output_tokens":1,"total_tokens":2}}})
}

fn summary() -> Value {
    json!({"type":"response.output_item.done","item":{"type":"message",
        "id":"summary","role":"assistant","content":[{"type":"output_text",
        "text":"Retained summary of earlier work."}]}})
}

async fn recovery_case(error: Value, websocket: bool, recoverable: bool, context: bool) {
    let home = TempDir::new().expect("code home");
    let project = TempDir::new().expect("project");
    let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
    let final_events = if recoverable { vec![completed()] } else { vec![error.clone()] };
    let responses = if context {
        vec![vec![error], vec![summary(), completed()], final_events]
    } else {
        vec![vec![error]]
    };
    let mut ws_task = None;
    let mut http_server = None;
    let base_url = if websocket {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let recorded = Arc::clone(&requests);
        ws_task = Some(tokio::spawn(async move {
            for events in responses {
                let (tcp, _) = listener.accept().await.expect("connection");
                let mut socket = tokio_tungstenite::accept_async(tcp).await.expect("handshake");
                let message = socket.next().await.expect("request").expect("message");
                recorded.lock().expect("requests").push(
                    serde_json::from_str(message.to_text().expect("text")).expect("JSON"));
                for event in events {
                    socket.send(Message::Text(event.to_string())).await.expect("send event");
                }
            }
        }));
        format!("http://{address}")
    } else {
        let server = MockServer::start().await;
        let expected = responses.len() as u64;
        let pending = Mutex::new(responses.into_iter());
        Mock::given(method("POST"))
            .respond_with(move |_: &wiremock::Request| {
                let events = pending.lock().expect("responses").next().expect("bounded requests");
                let body: String = events.iter().map(|event| format!("data: {event}\n\n")).collect();
                ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
                    .set_body_string(body)
            }).expect(expected).mount(&server).await;
        let uri = server.uri();
        http_server = Some(server);
        uri
    };
    let mut config = common::load_default_config_for_test(&home);
    config.cwd = project.path().to_path_buf();
    config.model = "gpt-6.1-sol".to_string();
    config.model_family = code_core::model_family::find_family_for_model(&config.model)
        .expect("modern family");
    config.model_auto_compact_token_limit = Some(i64::MAX);
    config.compact_prompt_override = Some("COMPACT_TEST: summarize earlier work".to_string());
    config.model_provider = built_in_model_providers(None)["openai"].clone();
    config.model_provider.base_url = Some(base_url);
    config.model_provider.wire_api = serde_json::from_value(json!(
        if websocket { "responses_websocket" } else { "responses" }
    )).expect("wire API");
    config.model_provider.supports_websockets = websocket;
    config.model_provider.request_max_retries = Some(0);
    // Permanent request errors must stop even when transient retries are available.
    config.model_provider.stream_max_retries = Some(if context { 0 } else { 3 });
    let conversation = ConversationManager::with_auth(CodexAuth::from_api_key("mock-key"))
        .new_conversation(config).await.expect("conversation").conversation;
    conversation.submit(Op::UserInput {
        items: vec![InputItem::Text { text: "PRESERVE_CURRENT_USER_TURN".to_string() }],
        final_output_json_schema: None,
    }).await.expect("submit");
    let events = timeout(Duration::from_secs(15), async {
        let mut events = Vec::new();
        loop {
            let event = conversation.next_event().await.expect("event").msg;
            let done = matches!(event, EventMsg::TaskComplete(_));
            events.push(event);
            if done { break events; }
        }
    }).await.expect("bounded turn");
    if let Some(task) = ws_task {
        timeout(Duration::from_secs(3), task).await.expect("bounded server").expect("server");
    }
    if let Some(server) = http_server {
        let recorded = server.received_requests().await.expect("HTTP requests");
        *requests.lock().expect("requests") = recorded.iter().map(|request|
            serde_json::from_slice(&request.body).expect("HTTP JSON")).collect();
    }
    let requests = requests.lock().expect("requests");
    assert_eq!(requests.len(), if context { 3 } else { 1 }, "events: {events:?}");
    for request in requests.iter() {
        assert_eq!(request["model"], "gpt-6.1-sol");
        assert!(!request.to_string().contains("codex-lite"));
    }
    if context {
        assert!(requests[1].to_string().contains("COMPACT_TEST"), "compaction request missing");
        assert!(requests[2].to_string().contains("PRESERVE_CURRENT_USER_TURN"));
        assert!(requests[2].to_string().contains("Retained summary"));
    }
    let errors: Vec<_> = events.iter().filter_map(|event| match event {
        EventMsg::Error(error) if !error.message.starts_with("Model hit context-window limit;") => {
            Some(error.message.as_str())
        }
        _ => None,
    }).collect();
    assert_eq!(errors.is_empty(), recoverable, "errors: {errors:?}");
    if context && !recoverable {
        assert!(errors.iter().any(|error| error.contains("context")), "{errors:?}");
    }
    conversation.submit(Op::Shutdown).await.expect("shutdown");
}

#[tokio::test]
async fn websocket_context_error_compacts_and_retries() {
    recovery_case(json!({"type":"error","error":{"type":"invalid_request_error",
        "code":"context_length_exceeded","message":"Input too large"}}), true, true, true).await;
}

#[tokio::test]
async fn websocket_status_context_error_compacts_and_retries() {
    recovery_case(json!({"type":"error","status":400,"error":{"type":"invalid_request_error",
        "code":"context_length_exceeded","message":"Input too large"}}), true, true, true).await;
}

#[tokio::test]
async fn websocket_failed_response_context_error_compacts_and_retries() {
    recovery_case(json!({"type":"response.failed","response":{"error":{
        "type":"invalid_request_error","code":"context_length_exceeded",
        "message":"Maximum context length exceeded"}}}), true, true, true).await;
}

#[tokio::test]
async fn websocket_invalid_parameter_is_terminal_without_compaction() {
    recovery_case(json!({"type":"error","error":{"type":"invalid_request_error",
        "code":"unsupported_parameter","message":"Invalid reasoning effort"}}), true, false, false).await;
}

#[tokio::test]
async fn websocket_context_parameter_rejection_is_not_context_exhaustion() {
    recovery_case(json!({"type":"error","error":{"type":"invalid_request_error",
        "code":"unsupported_value","message":"Maximum context length parameter is unsupported"}}),
        true, false, false).await;
}

#[tokio::test]
async fn websocket_context_code_without_error_type_compacts_and_retries() {
    recovery_case(json!({"type":"error","error":{"code":"context_length_exceeded",
        "message":"Input too large"}}), true, true, true).await;
}

#[tokio::test]
async fn http_context_code_without_message_pattern_compacts_and_retries() {
    recovery_case(json!({"type":"response.failed","response":{"error":{
        "code":"context_length_exceeded","message":"Input too large"}}}), false, true, true).await;
}

#[tokio::test]
async fn http_non_context_code_without_type_does_not_compact() {
    recovery_case(json!({"type":"response.failed","response":{"error":{
        "code":"unsupported_value","message":"Maximum context length parameter is unsupported"}}}),
        false, false, false).await;
}

#[tokio::test]
async fn websocket_non_context_code_without_type_does_not_compact() {
    recovery_case(json!({"type":"error","error":{
        "code":"unsupported_value","message":"Maximum context length parameter is unsupported"}}),
        true, false, false).await;
}

#[tokio::test]
async fn websocket_message_only_parameter_rejection_does_not_compact() {
    recovery_case(json!({"type":"error","error":{"type":"invalid_request_error",
        "message":"Maximum context length parameter is unsupported"}}), true, false, false).await;
}

#[tokio::test]
async fn http_message_only_parameter_rejection_does_not_compact() {
    recovery_case(json!({"type":"response.failed","response":{"error":{
        "type":"invalid_request_error","message":"Maximum context length parameter is unsupported"}}}),
        false, false, false).await;
}

#[tokio::test]
async fn websocket_context_message_without_code_compacts_and_retries() {
    recovery_case(json!({"type":"error","error":{"type":"invalid_request_error",
        "message":"Input exceeds the context window"}}), true, true, true).await;
}

#[tokio::test]
async fn websocket_context_error_after_compaction_is_bounded() {
    recovery_case(json!({"type":"error","error":{"type":"invalid_request_error",
        "code":"context_length_exceeded","message":"Input exceeds the context window"}}), true, false, true).await;
}

#[tokio::test]
async fn http_context_error_still_compacts_and_retries() {
    recovery_case(json!({"type":"response.failed","response":{"error":{
        "code":"context_length_exceeded","message":"Input exceeds the context window"}}}), false, true, true).await;
}
