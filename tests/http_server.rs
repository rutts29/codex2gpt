use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use codex2gpt::config::AppConfig;
use codex2gpt::http::router;
use codex2gpt::http::{
    AuthorizeRequest, ClientRegistrationRequest, HttpState, TokenExchangeRequest,
    authorization_approval_page, authorization_redirect_location, authorization_server_metadata,
    handle_authenticated_http_json_rpc, handle_authorize_request, handle_client_registration,
    handle_http_json_rpc, handle_token_exchange, healthz, protected_resource_metadata,
    www_authenticate_header,
};
use codex2gpt::mcp::AppState;
use serde_json::json;

fn state_with_workspace() -> AppState {
    let root = unique_temp_dir("http-root");
    let state_dir = unique_temp_dir("http-state");
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "allowed_workspaces": [
                {{"id": "repo", "path": "{}"}}
              ]
            }}"#,
            state_dir.display(),
            root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "codex2gpt-{name}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
    ));
    dir.push(format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn call_router(state: HttpState, request: Request<Body>) -> Response<Body> {
    let mut app = router(state);
    tower_service::Service::call(&mut app, request)
        .await
        .unwrap()
}

#[test]
fn healthz_returns_ok() {
    assert_eq!(healthz(), "ok");
}

#[test]
fn http_json_rpc_delegates_to_mcp_handler() {
    let response = handle_http_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list"}),
    );

    assert_eq!(response["id"], 7);
    assert!(response["result"]["tools"].as_array().unwrap().len() >= 3);
}

#[test]
fn authenticated_http_json_rpc_rejects_missing_or_invalid_bearer_token() {
    let state = HttpState::new(state_with_workspace(), "local-secret");

    let missing = handle_authenticated_http_json_rpc(
        &state,
        None,
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list"}),
    );
    let invalid = handle_authenticated_http_json_rpc(
        &state,
        Some("Bearer wrong"),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list"}),
    );

    assert_eq!(missing.0, StatusCode::UNAUTHORIZED);
    assert_eq!(invalid.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn mcp_post_unauthorized_emits_www_authenticate_resource_metadata_header() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");

    let response = call_router(
        state,
        Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            ))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.headers()[header::WWW_AUTHENTICATE],
        "Bearer resource_metadata=\"https://bridge.example.test/.well-known/oauth-protected-resource\""
    );
}

#[test]
fn www_authenticate_header_points_to_protected_resource_metadata_when_oauth_enabled() {
    let header = www_authenticate_header("https://bridge.example.test", true);

    assert_eq!(
        header,
        "Bearer resource_metadata=\"https://bridge.example.test/.well-known/oauth-protected-resource\""
    );
}

#[test]
fn www_authenticate_header_stays_plain_bearer_without_oauth() {
    assert_eq!(
        www_authenticate_header("https://bridge.example.test", false),
        "Bearer"
    );
}

#[test]
fn authenticated_http_json_rpc_accepts_valid_bearer_token() {
    let state = HttpState::new(state_with_workspace(), "local-secret");

    let response = handle_authenticated_http_json_rpc(
        &state,
        Some("Bearer local-secret"),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list"}),
    );

    assert_eq!(response.0, StatusCode::OK);
    assert!(response.1["result"]["tools"].as_array().unwrap().len() >= 3);
}

#[test]
fn authenticated_http_json_rpc_accepts_notifications_without_response_body() {
    let state = HttpState::new(state_with_workspace(), "local-secret");

    let response = handle_authenticated_http_json_rpc(
        &state,
        Some("Bearer local-secret"),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );

    assert_eq!(response.0, StatusCode::ACCEPTED);
    assert!(response.1.is_null());
}

#[test]
fn authenticated_http_json_rpc_accepts_oauth_issued_token() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_oauth_approval_token("approve-me");
    let resource = "http://127.0.0.1:8787/mcp".to_owned();
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/oauth/callback".to_owned()],
        },
    )
    .1;
    let client_id = registered["client_id"].as_str().unwrap().to_owned();
    let authorize = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: client_id.clone(),
            redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some(resource.clone()),
            approval_token: "approve-me".to_owned(),
        },
    );
    let code = authorize.1["code"].as_str().unwrap().to_owned();
    let token = handle_token_exchange(
        &state,
        TokenExchangeRequest {
            grant_type: "authorization_code".to_owned(),
            client_id,
            code,
            redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned(),
            resource: Some(resource),
        },
    )
    .1;
    let access_token = token["access_token"].as_str().unwrap();

    let response = handle_authenticated_http_json_rpc(
        &state,
        Some(&format!("Bearer {access_token}")),
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/list"}),
    );

    assert_eq!(response.0, StatusCode::OK);
}

#[test]
fn protected_resource_metadata_points_to_mcp_without_oauth_advertisement() {
    let metadata = protected_resource_metadata("https://bridge.example.test", false);

    assert_eq!(metadata["resource"], "https://bridge.example.test/mcp");
    assert!(metadata.get("authorization_servers").is_none());
}

#[test]
fn protected_resource_metadata_advertises_oauth_when_enabled() {
    let metadata = protected_resource_metadata("https://bridge.example.test", true);

    assert_eq!(metadata["resource"], "https://bridge.example.test/mcp");
    assert_eq!(
        metadata["authorization_servers"][0],
        "https://bridge.example.test"
    );
}

#[test]
fn authorization_server_metadata_advertises_pkce_dcr_flow() {
    let metadata = authorization_server_metadata("https://bridge.example.test");

    assert_eq!(metadata["issuer"], "https://bridge.example.test");
    assert_eq!(
        metadata["registration_endpoint"],
        "https://bridge.example.test/oauth/register"
    );
    assert_eq!(metadata["code_challenge_methods_supported"][0], "S256");
    assert_eq!(metadata["token_endpoint_auth_methods_supported"][0], "none");
}

#[test]
fn client_registration_returns_public_client_metadata() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_oauth_approval_token("approve-me");
    let response = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/oauth/callback".to_owned()],
        },
    );

    assert_eq!(response.0, StatusCode::CREATED);
    assert!(
        response.1["client_id"]
            .as_str()
            .unwrap()
            .starts_with("client-")
    );
    assert_eq!(response.1["token_endpoint_auth_method"], "none");
}

#[test]
fn client_registration_rejects_untrusted_redirect_uri() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_oauth_approval_token("approve-me");
    let response = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://evil.example/callback".to_owned()],
        },
    );

    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    assert_eq!(response.1["error"], "invalid_redirect_uri");
}

#[test]
fn client_registration_rejects_loopback_userinfo_redirect_uri() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_oauth_approval_token("approve-me");
    let response = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["http://localhost:8910@evil.example/callback".to_owned()],
        },
    );

    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    assert_eq!(response.1["error"], "invalid_redirect_uri");
}

#[test]
fn authorize_rejects_wrong_resource() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector/oauth/callback_123".to_owned()],
        },
    )
    .1;

    let response = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: registered["client_id"].as_str().unwrap().to_owned(),
            redirect_uri: "https://chatgpt.com/connector/oauth/callback_123".to_owned(),
            code_challenge: "challenge".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some("https://other.example.test/mcp".to_owned()),
            approval_token: "approve-me".to_owned(),
        },
    );

    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    assert_eq!(response.1["error"], "invalid_resource");
}

#[test]
fn authorize_rejects_missing_resource() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector/oauth/callback_123".to_owned()],
        },
    )
    .1;

    let response = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: registered["client_id"].as_str().unwrap().to_owned(),
            redirect_uri: "https://chatgpt.com/connector/oauth/callback_123".to_owned(),
            code_challenge: "challenge".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: None,
            approval_token: "approve-me".to_owned(),
        },
    );

    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    assert_eq!(response.1["error"], "invalid_resource");
}

#[test]
fn authorize_rejects_wrong_resource_before_local_approval() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector/oauth/callback_123".to_owned()],
        },
    )
    .1;

    let response = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: registered["client_id"].as_str().unwrap().to_owned(),
            redirect_uri: "https://chatgpt.com/connector/oauth/callback_123".to_owned(),
            code_challenge: "challenge".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some("https://other.example.test/mcp".to_owned()),
            approval_token: "wrong".to_owned(),
        },
    );

    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    assert_eq!(response.1["error"], "invalid_resource");
}

#[test]
fn authorize_requires_local_approval_token() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/oauth/callback".to_owned()],
        },
    )
    .1;

    let response = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: registered["client_id"].as_str().unwrap().to_owned(),
            redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
            code_challenge: "challenge".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some("http://127.0.0.1:8787/mcp".to_owned()),
            approval_token: "wrong".to_owned(),
        },
    );

    assert_eq!(response.0, StatusCode::UNAUTHORIZED);
}

#[test]
fn authorization_redirect_location_preserves_code_and_state() {
    let location = authorization_redirect_location(
        "https://chatgpt.com/oauth/callback",
        "code with spaces",
        Some("state/with/slash"),
    );

    assert_eq!(
        location,
        "https://chatgpt.com/oauth/callback?code=code%20with%20spaces&state=state%2Fwith%2Fslash"
    );
}

#[tokio::test]
async fn oauth_authorize_route_does_not_accept_query_string_approval_token() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector_platform_oauth_redirect".to_owned()],
        },
    )
    .1;
    let client_id = registered["client_id"].as_str().unwrap();
    let path = format!(
        "/oauth/authorize?response_type=code&client_id={client_id}&redirect_uri=https%3A%2F%2Fchatgpt.com%2Fconnector_platform_oauth_redirect&code_challenge=challenge&code_challenge_method=S256&state=chatgpt-state&resource=https%3A%2F%2Fbridge.example.test%2Fmcp&approval_token=approve-me"
    );

    let response = call_router(
        state,
        Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(header::LOCATION).is_none());
}

#[tokio::test]
async fn oauth_authorize_route_posts_local_approval_without_query_token() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector_platform_oauth_redirect".to_owned()],
        },
    )
    .1;
    let client_id = registered["client_id"].as_str().unwrap();
    let body = format!(
        "response_type=code&client_id={client_id}&redirect_uri=https%3A%2F%2Fchatgpt.com%2Fconnector_platform_oauth_redirect&code_challenge=challenge&code_challenge_method=S256&state=chatgpt-state&resource=https%3A%2F%2Fbridge.example.test%2Fmcp&approval_token=approve-me"
    );

    let response = call_router(
        state,
        Request::builder()
            .method("POST")
            .uri("/oauth/authorize")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::FOUND);
    let location = response.headers()[header::LOCATION].to_str().unwrap();
    assert!(
        location.starts_with("https://chatgpt.com/connector_platform_oauth_redirect?code=code-")
    );
    assert!(location.ends_with("&state=chatgpt-state"));
    assert!(!location.contains("approve-me"));
}

#[test]
fn authorization_approval_page_preserves_request_fields_and_escapes_html() {
    let page = authorization_approval_page(
        &AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: "client<&>".to_owned(),
            redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
            code_challenge: "challenge".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("state\"x".to_owned()),
            resource: Some("https://bridge.example.test/mcp".to_owned()),
            approval_token: String::new(),
        },
        false,
    );

    assert!(page.contains("name=\"approval_token\""));
    assert!(page.contains(r#"<form method="post" action="/oauth/authorize">"#));
    assert!(page.contains("client&lt;&amp;&gt;"));
    assert!(page.contains("state&quot;x"));
    assert!(page.contains("https://bridge.example.test/mcp"));
    assert!(page.contains("https://chatgpt.com/oauth/callback"));
}

#[test]
fn authorization_approval_page_shows_requestor_origin_and_rejection_banner() {
    let request = AuthorizeRequest {
        response_type: "code".to_owned(),
        client_id: "chatgpt".to_owned(),
        redirect_uri: "https://chatgpt.com/connector_platform_oauth_redirect".to_owned(),
        code_challenge: "challenge".to_owned(),
        code_challenge_method: "S256".to_owned(),
        state: None,
        resource: Some("https://bridge.example.test/mcp".to_owned()),
        approval_token: String::new(),
    };

    let fresh = authorization_approval_page(&request, false);
    assert!(fresh.contains("Requesting app"));
    assert!(fresh.contains("https://chatgpt.com</span>"));
    assert!(!fresh.contains("didn't match"));

    let rejected = authorization_approval_page(&request, true);
    assert!(rejected.contains("didn't match"));
    assert!(rejected.contains("Requesting app"));
}

#[test]
fn token_exchange_rejects_wrong_resource() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector/oauth/callback_123".to_owned()],
        },
    )
    .1;
    let client_id = registered["client_id"].as_str().unwrap().to_owned();
    let authorize = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: client_id.clone(),
            redirect_uri: "https://chatgpt.com/connector/oauth/callback_123".to_owned(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some("https://bridge.example.test/mcp".to_owned()),
            approval_token: "approve-me".to_owned(),
        },
    );
    let code = authorize.1["code"].as_str().unwrap().to_owned();

    let response = handle_token_exchange(
        &state,
        TokenExchangeRequest {
            grant_type: "authorization_code".to_owned(),
            client_id,
            code,
            redirect_uri: "https://chatgpt.com/connector/oauth/callback_123".to_owned(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned(),
            resource: Some("https://other.example.test/mcp".to_owned()),
        },
    );

    assert_eq!(response.0, StatusCode::BAD_REQUEST);
    assert_eq!(response.1["error"], "authorization code resource mismatch");
}

#[test]
fn token_exchange_accepts_matching_resource() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector/oauth/callback_123".to_owned()],
        },
    )
    .1;
    let client_id = registered["client_id"].as_str().unwrap().to_owned();
    let authorize = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: client_id.clone(),
            redirect_uri: "https://chatgpt.com/connector/oauth/callback_123".to_owned(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some("https://bridge.example.test/mcp".to_owned()),
            approval_token: "approve-me".to_owned(),
        },
    );
    let code = authorize.1["code"].as_str().unwrap().to_owned();

    let response = handle_token_exchange(
        &state,
        TokenExchangeRequest {
            grant_type: "authorization_code".to_owned(),
            client_id,
            code,
            redirect_uri: "https://chatgpt.com/connector/oauth/callback_123".to_owned(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned(),
            resource: Some("https://bridge.example.test/mcp".to_owned()),
        },
    );

    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.1["token_type"], "Bearer");
}

#[test]
fn client_registration_persists_for_reconnect_after_restart() {
    let clients_path = unique_temp_dir("oauth-clients").join("clients.json");
    let first_state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_clients_path(&clients_path)
        .with_oauth_approval_token("approve-me");
    let registered = handle_client_registration(
        &first_state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/connector_platform_oauth_redirect".to_owned()],
        },
    )
    .1;
    let client_id = registered["client_id"].as_str().unwrap().to_owned();

    let restarted_state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_clients_path(&clients_path)
        .with_oauth_approval_token("approve-me");

    let response = handle_authorize_request(
        &restarted_state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id,
            redirect_uri: "https://chatgpt.com/connector_platform_oauth_redirect".to_owned(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some("https://bridge.example.test/mcp".to_owned()),
            approval_token: "approve-me".to_owned(),
        },
    );

    assert_eq!(response.0, StatusCode::OK);
    assert!(response.1["code"].as_str().is_some());
}

#[test]
fn authorize_recovers_stale_chatgpt_client_after_restart() {
    let clients_path = unique_temp_dir("oauth-stale-client").join("clients.json");
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_base_url("https://bridge.example.test")
        .with_oauth_clients_path(&clients_path)
        .with_oauth_approval_token("approve-me");
    let client_id = "client-from-chatgpt-before-bridge-restart".to_owned();

    let authorize = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: client_id.clone(),
            redirect_uri: "https://chatgpt.com/connector_platform_oauth_redirect".to_owned(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some("https://bridge.example.test/mcp".to_owned()),
            approval_token: "approve-me".to_owned(),
        },
    );
    let code = authorize.1["code"].as_str().unwrap().to_owned();

    let token = handle_token_exchange(
        &state,
        TokenExchangeRequest {
            grant_type: "authorization_code".to_owned(),
            client_id,
            code,
            redirect_uri: "https://chatgpt.com/connector_platform_oauth_redirect".to_owned(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned(),
            resource: Some("https://bridge.example.test/mcp".to_owned()),
        },
    );

    assert_eq!(authorize.0, StatusCode::OK);
    assert_eq!(token.0, StatusCode::OK);
    assert_eq!(token.1["token_type"], "Bearer");
}

#[test]
fn token_exchange_rejects_reused_code() {
    let state = HttpState::new(state_with_workspace(), "local-secret")
        .with_oauth_approval_token("approve-me");
    let resource = "http://127.0.0.1:8787/mcp".to_owned();
    let registered = handle_client_registration(
        &state,
        ClientRegistrationRequest {
            redirect_uris: vec!["https://chatgpt.com/oauth/callback".to_owned()],
        },
    )
    .1;
    let client_id = registered["client_id"].as_str().unwrap().to_owned();
    let authorize = handle_authorize_request(
        &state,
        AuthorizeRequest {
            response_type: "code".to_owned(),
            client_id: client_id.clone(),
            redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
            code_challenge_method: "S256".to_owned(),
            state: Some("chatgpt-state".to_owned()),
            resource: Some(resource.clone()),
            approval_token: "approve-me".to_owned(),
        },
    );
    let code = authorize.1["code"].as_str().unwrap().to_owned();
    let first = handle_token_exchange(
        &state,
        TokenExchangeRequest {
            grant_type: "authorization_code".to_owned(),
            client_id: client_id.clone(),
            code: code.clone(),
            redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned(),
            resource: Some(resource.clone()),
        },
    );
    let second = handle_token_exchange(
        &state,
        TokenExchangeRequest {
            grant_type: "authorization_code".to_owned(),
            client_id,
            code,
            redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk".to_owned(),
            resource: Some(resource),
        },
    );

    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(second.0, StatusCode::BAD_REQUEST);
}
