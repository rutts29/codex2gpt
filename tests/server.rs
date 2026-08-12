use std::path::PathBuf;

use codex2gpt::config::{AppConfig, ToolSurface};
use codex2gpt::http::protected_resource_metadata_for_state;
use codex2gpt::server::{base_url_for_config, http_state_from_config, require_bearer_token};

#[test]
fn require_bearer_token_rejects_missing_or_blank_token() {
    assert!(require_bearer_token(None).is_err());
    assert!(require_bearer_token(Some("  ".to_owned())).is_err());
}

#[test]
fn require_bearer_token_accepts_non_blank_token() {
    assert_eq!(
        require_bearer_token(Some(" local-secret ".to_owned())).unwrap(),
        "local-secret"
    );
}

#[test]
fn base_url_defaults_to_configured_listen_addr() {
    let config = config_with_listen_addr("127.0.0.1:9999");

    assert_eq!(base_url_for_config(&config, None), "http://127.0.0.1:9999");
}

#[test]
fn base_url_uses_explicit_override() {
    let config = config_with_listen_addr("127.0.0.1:9999");

    assert_eq!(
        base_url_for_config(&config, Some("https://bridge.example.test/")),
        "https://bridge.example.test"
    );
}

#[test]
fn http_state_from_config_builds_authenticated_state() {
    let config = config_with_listen_addr("127.0.0.1:9999");
    let state = http_state_from_config(config, "local-secret", None, None);

    let response = codex2gpt::http::handle_authenticated_http_json_rpc(
        &state,
        Some("Bearer local-secret"),
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
    );

    assert_eq!(response.0, axum::http::StatusCode::OK);
}

#[test]
fn http_state_from_config_enables_oauth_with_approval_token() {
    let config = config_with_listen_addr("127.0.0.1:9999");
    let state = http_state_from_config(
        config,
        "local-secret",
        Some("https://bridge.example.test"),
        Some("approve-me"),
    );

    let metadata = protected_resource_metadata_for_state(&state);

    assert_eq!(
        metadata["authorization_servers"][0],
        "https://bridge.example.test"
    );
}

fn config_with_listen_addr(listen_addr: &str) -> AppConfig {
    AppConfig {
        listen_addr: listen_addr.to_owned(),
        state_dir: PathBuf::from("/tmp/codex2gpt-state"),
        codex_binary: "codex".to_owned(),
        git_binary: "git".to_owned(),
        rg_binary: "rg".to_owned(),
        max_read_bytes: 64 * 1024,
        max_search_results: 100,
        widget_domain: None,
        tool_surface: ToolSurface::Full,
        allowed_workspaces: Vec::new(),
    }
}
