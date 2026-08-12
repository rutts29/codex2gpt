use std::error::Error;
use std::path::Path;

use crate::config::AppConfig;
use crate::http::{HttpState, router};
use crate::mcp::AppState;

pub fn require_bearer_token(value: Option<String>) -> Result<String, String> {
    let Some(token) = value else {
        return Err("CODEX2GPT_BEARER_TOKEN is required".to_owned());
    };
    let token = token.trim();
    if token.is_empty() {
        return Err("CODEX2GPT_BEARER_TOKEN must not be blank".to_owned());
    }

    Ok(token.to_owned())
}

pub fn base_url_for_config(config: &AppConfig, override_url: Option<&str>) -> String {
    let url = override_url.unwrap_or_else(|| config.listen_addr.as_str());
    let url = url.trim_end_matches('/');
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_owned()
    } else {
        format!("http://{url}")
    }
}

pub fn http_state_from_config(
    config: AppConfig,
    bearer_token: &str,
    base_url: Option<&str>,
    oauth_approval_token: Option<&str>,
) -> HttpState {
    let base_url = base_url_for_config(&config, base_url);
    let oauth_clients_path = config.state_dir.join("oauth-clients.json");
    let state = HttpState::new(AppState::new(config), bearer_token)
        .with_base_url(base_url)
        .with_oauth_clients_path(oauth_clients_path);
    match oauth_approval_token {
        Some(token) => state.with_oauth_approval_token(token),
        None => state,
    }
}

pub async fn serve(config_path: &Path) -> Result<(), Box<dyn Error>> {
    let config = AppConfig::load_from_file(config_path)?;
    let bearer_token = require_bearer_token(std::env::var("CODEX2GPT_BEARER_TOKEN").ok())?;
    let base_url = std::env::var("CODEX2GPT_BASE_URL").ok();
    let oauth_approval_token = std::env::var("CODEX2GPT_OAUTH_APPROVAL_TOKEN").ok();
    let listen_addr = config.listen_addr.clone();
    let state = http_state_from_config(
        config,
        &bearer_token,
        base_url.as_deref(),
        oauth_approval_token.as_deref(),
    );
    let listener = tokio::net::TcpListener::bind(&listen_addr).await?;

    tracing::info!("listening on {listen_addr}");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
