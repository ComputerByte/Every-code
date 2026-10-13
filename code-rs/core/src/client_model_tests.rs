//! Mock transport tests exercise the same serialized request paths as production.
use super::*;
use crate::config::{Config, ConfigOverrides, ConfigToml};
use crate::modern_models::MODERN_MODELS;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_for(base_url: String, model: &str, oauth: bool) -> ModelClient {
    let home = tempdir().expect("isolated code home");
    let mut config = Config::load_from_base_config_with_overrides(
        ConfigToml::default(), ConfigOverrides::default(), home.path().to_path_buf(),
    ).expect("config");
    config.model = model.to_string();
    config.model_family = find_family_for_model(model).unwrap_or_else(|| derive_default_model_family(model));
    config.model_provider_id = "openai".to_string();
    let mut provider = config.model_provider.clone();
    provider.base_url = Some(base_url);
    provider.requires_openai_auth = false;
    provider.request_max_retries = Some(4);
    provider.stream_max_retries = Some(4);
    provider.supports_websockets = true;
    let auth = oauth.then(|| AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing()));
    ModelClient::new(Arc::new(config), auth, None, provider,
        ReasoningEffortConfig::Max, ReasoningSummaryConfig::None,
        TextVerbosityConfig::Medium, Uuid::new_v4(),
        Arc::new(Mutex::new(DebugLogger::new(false).expect("debug logger"))))
}

fn completion() -> Value {
    serde_json::json!({"type":"response.completed", "response":{"id":"resp_mock","usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}})
}

fn prompt() -> Prompt {
    let mut prompt = Prompt::default();
    prompt.store = true;
    prompt.include_additional_instructions = false;
    prompt.input = vec![ResponseItem::Message {
        id: None, role: "user".to_string(),
        content: vec![ContentItem::InputText { text: "mock request".to_string() }],
        end_turn: None, phase: None,
    }];
    prompt.tools = vec![crate::agent_tool::create_agent_tool(&["code-gpt-6.1-sol".to_string()])];
    prompt
}

async fn assert_completed(mut stream: ResponseStream) {
    while let Some(event) = timeout(Duration::from_secs(3), stream.next()).await.expect("bounded stream") {
        if matches!(event.expect("stream event"), ResponseEvent::Completed { .. }) { return; }
    }
    panic!("missing completed event");
}

fn assert_lite_payload(body: &Value, model: &str) {
    assert_eq!(body["model"], model);
    assert_eq!(body["store"], false);
    assert_eq!(body["parallel_tool_calls"], false);
    assert_eq!(body["reasoning"]["effort"], "max");
    assert_eq!(body["reasoning"]["context"], "all_turns");
    assert!(body["reasoning"].get("summary").is_none());
    assert_eq!(body["include"], serde_json::json!(["reasoning.encrypted_content"]));
    assert!(!body.to_string().contains("codex-lite"));
    assert_eq!(body["input"][0]["type"], "additional_tools");
    assert!(body["input"][0]["tools"].to_string().contains("agent"));
}

#[tokio::test]
async fn modern_models_serialize_lite_http_requests() {
    for spec in MODERN_MODELS {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/responses"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
                .set_body_string(format!("data: {}\n\n", completion())))
            .expect(1).mount(&server).await;
        let client = client_for(server.uri(), spec.id, true);
        let family = find_family_for_model(spec.id).expect("model family");
        assert_completed(client.stream_responses_for_family(&prompt(), None, family).await.expect("stream")).await;
        let requests = server.received_requests().await.expect("requests");
        assert_eq!(requests[0].headers.get(X_OPENAI_INTERNAL_CODEX_RESPONSES_LITE_HEADER).expect("lite header"), "true");
        assert_lite_payload(&serde_json::from_slice(&requests[0].body).expect("JSON body"), spec.id);
    }
}

#[tokio::test]
async fn custom_provider_does_not_inherit_lite_and_override_is_exact() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
            .set_body_string(format!("data: {}\n\n", completion())))
        .expect(1).mount(&server).await;
    let mut client = client_for(server.uri(), "gpt-5.5", false);
    client.provider.supports_websockets = false;
    client.summary = ReasoningSummaryConfig::Detailed;
    let mut prompt = prompt();
    prompt.model_override = Some("gpt-6.1-sol".to_string());
    prompt.model_family_override = find_family_for_model("gpt-6.1-sol");
    assert_completed(client.stream(&prompt).await.expect("custom stream")).await;
    let requests = server.received_requests().await.expect("requests");
    assert!(requests[0].headers.get(X_OPENAI_INTERNAL_CODEX_RESPONSES_LITE_HEADER).is_none());
    let body: Value = serde_json::from_slice(&requests[0].body).expect("body");
    assert_eq!(body["model"], "gpt-6.1-sol");
    assert_eq!(body["store"], true);
    assert!(body["reasoning"].is_null());
    let family = client.model_family_for_request(&prompt, "gpt-6.1-sol");
    assert_eq!(family.context_window, None);
    assert_eq!(family.max_output_tokens, None);
    assert!(!family.supports_reasoning_summaries);
    assert!(!family.supports_search_tool);
    assert!(body["tools"].to_string().contains("agent"));
}

#[tokio::test]
async fn bad_request_is_not_retried_by_http_transport() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).respond_with(ResponseTemplate::new(400).set_body_json(
        serde_json::json!({"error":{"type":"invalid_request_error","message":"unsupported model"}})))
        .expect(1).mount(&server).await;
    let client = client_for(server.uri(), "gpt-6.1-sol", false);
    let result = timeout(Duration::from_secs(3), client.stream_responses(&prompt(), None)).await.expect("no retry delay");
    assert!(matches!(result, Err(CodexErr::UnexpectedStatus(_))));
    assert_eq!(server.received_requests().await.expect("requests").len(), 1);
}

#[tokio::test]
async fn modern_models_serialize_lite_websocket_requests() {
    for spec in MODERN_MODELS {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let addr = listener.local_addr().expect("address");
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.expect("connection");
            let mut socket = tokio_tungstenite::accept_hdr_async(tcp, |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
                assert_eq!(request.headers().get(X_OPENAI_INTERNAL_CODEX_RESPONSES_LITE_HEADER).expect("lite header"), "true");
                Ok(response)
            }).await.expect("handshake");
            let payload = socket.next().await.expect("message").expect("message");
            let body: Value = serde_json::from_str(payload.to_text().expect("text")).expect("JSON");
            socket.send(Message::Text(completion().to_string())).await.expect("completion");
            body
        });
        let client = client_for(format!("http://{addr}"), spec.id, true);
        let family = find_family_for_model(spec.id).expect("family");
        let stream = client.stream_responses_websocket_for_family(&prompt(), None, ResponsesWebsocketVersion::V2, family).await.expect("stream");
        assert_completed(stream).await;
        let body = server.await.expect("server");
        assert_eq!(body["type"], "response.create");
        assert_lite_payload(&body, spec.id);
    }
}

#[tokio::test]
async fn websocket_bad_request_does_not_retry_or_fall_back_to_http() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(400).set_body_string("unsupported parameter"))
        .expect(1).mount(&server).await;
    let mut client = client_for(server.uri(), "gpt-6.1-sol", false);
    client.provider.wire_api = WireApi::ResponsesWebsocket;
    let result = timeout(Duration::from_secs(3), client.stream(&prompt())).await.expect("bounded failure");
    assert!(matches!(result, Err(CodexErr::UnexpectedStatus(_))));
    let requests = server.received_requests().await.expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
}

#[tokio::test]
async fn unsupported_first_party_model_has_actionable_diagnostic() {
    let client = client_for("https://api.openai.com/v1".to_string(), "gpt-6-terra", false);
    match client.stream(&prompt()).await {
        Err(CodexErr::UnsupportedOperation(message)) => assert!(message.contains("unverified OpenAI model 'gpt-6-terra'")),
        _ => panic!("unverified model must fail before network I/O"),
    }
}

#[tokio::test]
async fn websocket_route_not_found_falls_back_once_to_http() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(404)).expect(1).mount(&server).await;
    Mock::given(method("POST")).and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
            .set_body_string(format!("data: {}\n\n", completion())))
        .expect(1).mount(&server).await;
    let mut client = client_for(server.uri(), "gpt-6.1-sol", false);
    client.provider.wire_api = WireApi::ResponsesWebsocket;
    assert_completed(client.stream(&prompt()).await.expect("HTTP fallback")).await;
}

#[tokio::test]
async fn disabling_summary_preserves_lite_context_and_oauth_compaction_limit() {
    let server = MockServer::start().await;
    Mock::given(method("POST")).and(path("/responses"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
            .set_body_string(format!("data: {}\n\n", completion())))
        .expect(1).mount(&server).await;
    let client = client_for(server.uri(), "gpt-6.1-sol", true);
    client.disable_reasoning_summary();
    assert_completed(client.stream_responses_for_family(&prompt(), None,
        find_family_for_model("gpt-6.1-sol").expect("family")).await.expect("stream")).await;
    let requests = server.received_requests().await.expect("requests");
    assert_lite_payload(&serde_json::from_slice(&requests[0].body).expect("JSON"), "gpt-6.1-sol");
    let mut client = client;
    client.provider.base_url = None;
    assert!(client.get_auto_compact_token_limit().unwrap() <= 784_800);
}

/// Opt-in live check: reads existing credentials into memory, uses externally
/// managed auth (no refresh/write), and never logs credentials or persists them.
#[tokio::test]
#[ignore = "requires explicitly authorized existing ChatGPT credentials"]
async fn authenticated_gpt_6_1_sol_executes_without_substitution() {
    let auth_path = std::env::var_os("EVERY_CODE_SMOKE_AUTH_PATH").expect("set existing auth file path");
    let auth = crate::auth::try_read_auth_json(std::path::Path::new(&auth_path)).expect("read auth");
    let tokens = auth.tokens.expect("existing OAuth tokens");
    let managed = CodexAuth::from_tokens_with_originator_and_mode(tokens, auth.last_refresh,
        "every-code-model-validation", code_app_server_protocol::AuthMode::ChatgptAuthTokens);
    let mut client = client_for("https://api.openai.com/v1".to_string(), "gpt-6.1-sol", true);
    client.auth_manager = Some(AuthManager::from_auth_for_testing(managed));
    client.provider.base_url = None;
    client.provider.supports_websockets = false;
    client.effort = ReasoningEffortConfig::Low;
    client.provider.request_max_retries = Some(0);
    client.provider.stream_max_retries = Some(0);
    let mut request = prompt();
    request.tools.clear();
    request.input = vec![ResponseItem::Message {
        id: None, role: "user".to_string(),
        content: vec![ContentItem::InputText { text: "Reply with exactly MODEL_OK. Do not use tools.".to_string() }],
        end_turn: None, phase: None,
    }];
    let mut stream = timeout(Duration::from_secs(60), client.stream(&request)).await.expect("bounded request").expect("live stream");
    let mut text = String::new();
    let mut completed = false;
    while let Some(event) = timeout(Duration::from_secs(60), stream.next()).await.expect("bounded stream") {
        match event.expect("live event") {
            ResponseEvent::Created { response_model: Some(model), .. } => assert_eq!(model, "gpt-6.1-sol"),
            ResponseEvent::OutputTextDelta { delta, .. } => text.push_str(&delta),
            ResponseEvent::Completed { .. } => { completed = true; break; },
            _ => {},
        }
    }
    assert!(completed);
    assert_eq!(text.trim(), "MODEL_OK");
}

#[test]
fn modern_backend_protocol_floor_is_scoped_to_oauth() {
    for oauth in [false, true] {
        let client = client_for("https://api.openai.com/v1".to_string(), "gpt-6.1-sol", oauth);
        let request = client.apply_requested_model_headers(
            reqwest::Client::new().post("https://example.invalid/responses"), "gpt-6.1-sol").build().expect("request");
        let expected = if oauth { code_version::codex_backend_version_for_model("gpt-6.1-sol") }
            else { code_version::wire_compatible_version_for_model("gpt-6.1-sol") };
        assert_eq!(request.headers()["version"], expected);
    }
    let client = client_for("https://proxy.example/v1".to_string(), "gpt-6.1-sol", true);
    let request = client.apply_requested_model_headers(
        reqwest::Client::new().post("https://proxy.example/v1/responses"), "gpt-6.1-sol").build().expect("request");
    assert_eq!(request.headers()["version"], code_version::wire_compatible_version_for_model("gpt-6.1-sol"));
}

struct RefreshEndpointReset(Option<std::ffi::OsString>);

#[test]
fn websocket_diagnostic_headers_redact_credentials() {
    let mut headers = HeaderMap::new();
    for name in ["authorization", "proxy-authorization", "cookie", "api-key", "x-api-key"] {
        headers.insert(name, reqwest::header::HeaderValue::from_static("mock-secret-token"));
    }
    headers.insert("version", reqwest::header::HeaderValue::from_static("0.162.1"));
    let snapshot = header_map_to_json(&headers);
    assert!(!snapshot.to_string().contains("mock-secret-token"));
    assert_eq!(snapshot["authorization"][0], "[REDACTED]");
    assert_eq!(snapshot["version"][0], "0.162.1");
    assert_eq!(headers["authorization"], "mock-secret-token");
}

impl Drop for RefreshEndpointReset {
    fn drop(&mut self) {
        unsafe {
            match self.0.take() {
                Some(value) => std::env::set_var(auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR, value),
                None => std::env::remove_var(auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR),
            }
        }
    }
}

// accept_hdr_async requires an unboxed HTTP response as the callback error.
#[allow(clippy::result_large_err)]
async fn websocket_oauth_recovery_case(refresh_succeeds: bool, remains_unauthorized: bool, lite: bool) {
    let home = tempdir().expect("isolated auth home");
    let refresh_server = MockServer::start().await;
    let refresh_response = if refresh_succeeds {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token":"fresh-mock-token", "refresh_token":"fresh-mock-refresh"}))
    } else {
        ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error":{"code":"refresh_token_expired","message":"Sign in again"}}))
    };
    Mock::given(method("POST")).and(path("/token"))
        .respond_with(refresh_response).expect(1).mount(&refresh_server).await;
    let _reset = RefreshEndpointReset(std::env::var_os(auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR));
    unsafe {
        std::env::set_var(auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR, format!("{}/token", refresh_server.uri()));
    }
    let mut tokens = CodexAuth::create_dummy_chatgpt_auth_for_testing()
        .get_token_data().await.expect("dummy tokens");
    tokens.id_token.raw_jwt = "e30.e30.signature".to_string();
    tokens.account_id = None;
    tokens.access_token = "expired-mock-token".to_string();
    auth::write_auth_json(&auth::get_auth_file(home.path()), &auth::AuthDotJson {
        auth_mode: Some(code_app_server_protocol::AuthMode::ChatGPT),
        openai_api_key: None, tokens: Some(tokens), last_refresh: Some(Utc::now()),
    }).expect("write isolated credentials");
    let manager = AuthManager::new(home.path().to_path_buf(),
        code_app_server_protocol::AuthMode::ChatGPT, "repair-test".to_string());
    assert!(manager.auth().is_some());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.expect("initial connection");
        let first = tokio_tungstenite::accept_hdr_async(tcp, |request: &tokio_tungstenite::tungstenite::handshake::server::Request, _response| {
            assert_eq!(request.headers()["authorization"], "Bearer expired-mock-token");
            Err(tokio_tungstenite::tungstenite::http::Response::builder().status(401)
                .body(Some("expired credential".to_string())).expect("401"))
        }).await;
        assert!(first.is_err());
        if !refresh_succeeds { return None; }
        let (tcp, _) = timeout(Duration::from_secs(3), listener.accept()).await
            .expect("retry only after refresh").expect("retry connection");
        let second = tokio_tungstenite::accept_hdr_async(tcp, |request: &tokio_tungstenite::tungstenite::handshake::server::Request, response| {
            assert_eq!(request.headers()["authorization"], "Bearer fresh-mock-token");
            if remains_unauthorized {
                Err(tokio_tungstenite::tungstenite::http::Response::builder().status(401)
                    .body(Some("credential rejected".to_string())).expect("401"))
            } else { Ok(response) }
        }).await;
        if remains_unauthorized { assert!(second.is_err()); return None; }
        let mut socket = second.expect("refreshed handshake");
        let message = socket.next().await.expect("request").expect("message");
        let body: Value = serde_json::from_str(message.to_text().expect("text")).expect("JSON");
        socket.send(Message::Text(completion().to_string())).await.expect("complete");
        Some(body)
    });
    let mut client = client_for(format!("http://{address}"), "gpt-6.1-sol", true);
    client.auth_manager = Some(Arc::new(manager));
    client.provider.requires_openai_auth = true;
    client.provider.wire_api = WireApi::ResponsesWebsocket;
    // A transport retry budget of zero must still allow the one credential recovery.
    client.provider.request_max_retries = Some(0);
    let request = prompt();
    let result = if lite {
        timeout(Duration::from_secs(5), client.stream_responses_websocket_for_family(
            &request, None, ResponsesWebsocketVersion::V2,
            find_family_for_model("gpt-6.1-sol").expect("family"),
        )).await.expect("bounded Lite recovery")
    } else {
        timeout(Duration::from_secs(5), client.stream(&request)).await.expect("bounded recovery")
    };
    if refresh_succeeds && !remains_unauthorized {
        assert_completed(result.expect("refreshed stream")).await;
    } else {
        match result {
            Err(CodexErr::AuthRefreshPermanent(message)) => assert!(message.contains("again")),
            _ => panic!("expected actionable terminal authentication error"),
        }
    }
    if let Some(body) = timeout(Duration::from_secs(4), server).await.expect("server timeout").expect("server") {
        assert_eq!(body["model"], "gpt-6.1-sol");
        if lite {
            assert_lite_payload(&body, "gpt-6.1-sol");
        } else {
            // A localhost provider has conservative metadata even with OAuth.
            assert!(body["reasoning"].is_null());
        }
        assert!(!body.to_string().contains("codex-lite"));
    }
}

#[tokio::test]
#[serial_test::serial]
async fn websocket_oauth_401_refreshes_and_reconnects() {
    websocket_oauth_recovery_case(true, false, false).await;
}

#[tokio::test]
#[serial_test::serial]
async fn websocket_oauth_401_preserves_lite_payload_after_refresh() {
    websocket_oauth_recovery_case(true, false, true).await;
}

#[tokio::test]
#[serial_test::serial]
async fn websocket_oauth_401_refresh_failure_is_terminal() {
    websocket_oauth_recovery_case(false, false, false).await;
}

#[tokio::test]
#[serial_test::serial]
async fn websocket_oauth_401_after_refresh_is_terminal() {
    websocket_oauth_recovery_case(true, true, false).await;
}

#[tokio::test]
async fn websocket_custom_provider_401_does_not_refresh_openai_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(401))
        .expect(1).mount(&server).await;
    let mut client = client_for(server.uri(), "gpt-6.1-sol", true);
    Arc::make_mut(&mut client.config).model_provider_id = "custom".to_string();
    client.provider.requires_openai_auth = true;
    client.provider.wire_api = WireApi::ResponsesWebsocket;
    let result = timeout(Duration::from_secs(3), client.stream(&prompt())).await.expect("bounded failure");
    assert!(matches!(result, Err(CodexErr::UnexpectedStatus(error)) if error.status == StatusCode::UNAUTHORIZED));
}

#[tokio::test]
async fn modern_public_api_models_serialize_standard_responses() {
    for spec in MODERN_MODELS {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/responses"))
            .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream")
                .set_body_string(format!("data: {}\n\n", completion())))
            .expect(1).mount(&server).await;
        let mut client = client_for("https://api.openai.com/v1".to_string(), spec.id, false);
        let family = client.normalize_model_family_for_provider(find_family_for_model(spec.id).expect("family"));
        client.provider.base_url = Some(server.uri());
        assert_completed(client.stream_responses_for_family(&prompt(), None, family).await.expect("stream")).await;
        let requests = server.received_requests().await.expect("requests");
        let body: Value = serde_json::from_slice(&requests[0].body).expect("body");
        assert_eq!(body["model"], spec.id);
        assert_eq!(body["reasoning"]["effort"], "max");
        assert!(body["reasoning"].get("context").is_none());
        assert_eq!(body["store"], true);
        assert!(requests[0].headers.get(X_OPENAI_INTERNAL_CODEX_RESPONSES_LITE_HEADER).is_none());
    }
}
