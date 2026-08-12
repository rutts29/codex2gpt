use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::mcp::{AppState, handle_json_rpc};
use crate::oauth::{
    ACCESS_TOKEN_TTL_SECONDS, AuthorizationCode, BearerGate, ClientRegistration, OAuthStore,
    TokenRequest, is_allowed_redirect_uri,
};

#[derive(Clone, Debug)]
pub struct HttpState {
    app: AppState,
    bearer_gate: BearerGate,
    base_url: String,
    oauth_approval_gate: Option<BearerGate>,
    oauth_store: Arc<Mutex<OAuthStore>>,
    oauth_clients_path: Option<PathBuf>,
}

impl HttpState {
    pub fn new(app: AppState, bearer_token: &str) -> Self {
        Self {
            app,
            bearer_gate: BearerGate::new(bearer_token),
            base_url: "http://127.0.0.1:8787".to_owned(),
            oauth_approval_gate: None,
            oauth_store: Arc::new(Mutex::new(OAuthStore::default())),
            oauth_clients_path: None,
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_oauth_approval_token(mut self, token: &str) -> Self {
        self.oauth_approval_gate = Some(BearerGate::new(token));
        self
    }

    pub fn with_oauth_clients_path(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        match OAuthStore::load_clients(&path) {
            Ok(store) => {
                self.oauth_store = Arc::new(Mutex::new(store));
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "failed to load oauth clients");
            }
        }
        self.oauth_clients_path = Some(path);
        self
    }

    fn oauth_enabled(&self) -> bool {
        self.oauth_approval_gate.is_some()
    }

    fn save_oauth_clients(&self, store: &OAuthStore) -> Result<(), String> {
        let Some(path) = &self.oauth_clients_path else {
            return Ok(());
        };
        store
            .save_clients(path)
            .map_err(|error| format!("failed to save oauth clients: {error}"))
    }
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/mcp", post(mcp_post))
        .route("/oauth/register", post(oauth_register_post))
        .route(
            "/oauth/authorize",
            get(oauth_authorize_get).post(oauth_authorize_post),
        )
        .route("/oauth/token", post(oauth_token_post))
        .route(
            "/.well-known/oauth-protected-resource",
            get(protected_resource_handler),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(authorization_server_handler),
        )
        .with_state(state)
}

pub fn healthz() -> &'static str {
    "ok"
}

pub fn handle_http_json_rpc(state: &AppState, request: Value) -> Value {
    handle_json_rpc(state, request)
}

pub fn handle_authenticated_http_json_rpc(
    state: &HttpState,
    authorization_header: Option<&str>,
    request: Value,
) -> (StatusCode, Value) {
    if !state.bearer_gate.allows(authorization_header) && !state.oauth_allows(authorization_header)
    {
        return (StatusCode::UNAUTHORIZED, json!({"error": "unauthorized"}));
    }

    let response = handle_http_json_rpc(&state.app, request);
    if response.is_null() {
        return (StatusCode::ACCEPTED, response);
    }

    (StatusCode::OK, response)
}

impl HttpState {
    fn oauth_allows(&self, authorization_header: Option<&str>) -> bool {
        let Some(header) = authorization_header else {
            return false;
        };
        let Some(token) = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
        else {
            return false;
        };
        self.oauth_store
            .lock()
            .expect("oauth store mutex poisoned")
            .validate_token(token)
    }
}

pub fn protected_resource_metadata(base_url: &str, oauth_enabled: bool) -> Value {
    let base = base_url.trim_end_matches('/');
    if oauth_enabled {
        json!({
            "resource": oauth_resource(base_url),
            "authorization_servers": [base]
        })
    } else {
        json!({
            "resource": oauth_resource(base_url)
        })
    }
}

pub fn protected_resource_metadata_for_state(state: &HttpState) -> Value {
    protected_resource_metadata(&state.base_url, state.oauth_enabled())
}

pub fn www_authenticate_header(base_url: &str, oauth_enabled: bool) -> String {
    if oauth_enabled {
        format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource\"",
            base_url.trim_end_matches('/')
        )
    } else {
        "Bearer".to_owned()
    }
}

fn oauth_resource(base_url: &str) -> String {
    format!("{}/mcp", base_url.trim_end_matches('/'))
}

pub fn authorization_server_metadata(base_url: &str) -> Value {
    let base = base_url.trim_end_matches('/');
    json!({
        "issuer": base,
        "authorization_endpoint": format!("{base}/oauth/authorize"),
        "token_endpoint": format!("{base}/oauth/token"),
        "registration_endpoint": format!("{base}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"]
    })
}

#[derive(Debug, Deserialize)]
pub struct ClientRegistrationRequest {
    pub redirect_uris: Vec<String>,
}

pub fn handle_client_registration(
    state: &HttpState,
    request: ClientRegistrationRequest,
) -> (StatusCode, Value) {
    if !state.oauth_enabled() || request.redirect_uris.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            json!({"error": "invalid_client_metadata"}),
        );
    }
    if request
        .redirect_uris
        .iter()
        .any(|redirect_uri| !is_allowed_redirect_uri(redirect_uri))
    {
        return (
            StatusCode::BAD_REQUEST,
            json!({"error": "invalid_redirect_uri"}),
        );
    }
    let mut store = state
        .oauth_store
        .lock()
        .expect("oauth store mutex poisoned");
    let client = store.register_client(ClientRegistration {
        redirect_uris: request.redirect_uris,
    });
    if let Err(error) = state.save_oauth_clients(&store) {
        tracing::error!(%error);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": "oauth_client_persistence_failed"}),
        );
    }

    (
        StatusCode::CREATED,
        json!({
            "client_id": client.client_id,
            "redirect_uris": client.redirect_uris,
            "grant_types": ["authorization_code"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none"
        }),
    )
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub state: Option<String>,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub approval_token: String,
}

pub fn handle_authorize_request(
    state: &HttpState,
    request: AuthorizeRequest,
) -> (StatusCode, Value) {
    let Some(gate) = &state.oauth_approval_gate else {
        return (StatusCode::BAD_REQUEST, json!({"error": "oauth_disabled"}));
    };
    let expected_resource = oauth_resource(&state.base_url);
    if request.resource.as_deref() != Some(expected_resource.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            json!({"error": "invalid_resource"}),
        );
    }
    if !gate.allows(Some(&format!("Bearer {}", request.approval_token))) {
        return (
            StatusCode::UNAUTHORIZED,
            json!({"error": "approval_required"}),
        );
    }
    if request.response_type != "code" || request.code_challenge_method != "S256" {
        return (
            StatusCode::BAD_REQUEST,
            json!({"error": "unsupported_authorization_request"}),
        );
    }
    let code = new_url_token("code");
    let mut store = state
        .oauth_store
        .lock()
        .expect("oauth store mutex poisoned");
    if !store.redirect_uri_allowed(&request.client_id, &request.redirect_uri) {
        if !store.ensure_client_with_redirect_uri(&request.client_id, &request.redirect_uri) {
            return (
                StatusCode::BAD_REQUEST,
                json!({"error": "invalid_redirect_uri"}),
            );
        }
        if let Err(error) = state.save_oauth_clients(&store) {
            tracing::error!(%error);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": "oauth_client_persistence_failed"}),
            );
        }
    }
    store.insert_code(AuthorizationCode {
        code: code.clone(),
        client_id: request.client_id,
        redirect_uri: request.redirect_uri,
        code_challenge: request.code_challenge,
        resource: request.resource,
    });

    (
        StatusCode::OK,
        json!({
            "code": code,
            "state": request.state
        }),
    )
}

pub fn authorization_redirect_location(
    redirect_uri: &str,
    code: &str,
    state: Option<&str>,
) -> String {
    let separator = if redirect_uri.contains('?') { '&' } else { '?' };
    let mut location = format!("{redirect_uri}{separator}code={}", percent_encode(code));
    if let Some(state) = state {
        location.push_str("&state=");
        location.push_str(&percent_encode(state));
    }
    location
}

const AUTH_PAGE_CSS: &str = r#"
    * { box-sizing: border-box; }
    body { margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 24px; font: 15px/1.5 -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", system-ui, sans-serif; -webkit-font-smoothing: antialiased; color: var(--fg); background-color: var(--bg); background-image: var(--aura); background-repeat: no-repeat; background-attachment: fixed; }
    .card { width: 100%; max-width: 380px; background: var(--glass-bg); -webkit-backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-sat)); backdrop-filter: blur(var(--glass-blur)) saturate(var(--glass-sat)); border: 1px solid var(--glass-border); border-radius: 18px; padding: 28px; box-shadow: var(--glass-rim), var(--shadow); }
    .glyph { width: 40px; height: 40px; border-radius: 10px; display: grid; place-items: center; background: var(--accent); color: var(--on-accent); margin-bottom: 16px; box-shadow: inset 0 1px 0 rgba(255,255,255,.45), 0 6px 16px rgba(0,122,255,.35); }
    .glyph svg { width: 22px; height: 22px; }
    h1 { font-size: 20px; font-weight: 600; letter-spacing: -0.02em; margin: 0 0 8px; }
    .lede { margin: 0 0 18px; color: var(--muted); font-size: 14px; }
    .lede strong { color: var(--fg-2); font-weight: 600; }
    .requestor { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 10px 12px; border: 1px solid var(--glass-border); border-radius: var(--radius-sm); background: var(--glass-bg-2); -webkit-backdrop-filter: blur(8px) saturate(var(--glass-sat)); backdrop-filter: blur(8px) saturate(var(--glass-sat)); margin-bottom: 16px; font-size: 13px; box-shadow: var(--glass-rim); }
    .requestor .k { color: var(--muted); }
    .requestor .v { color: var(--fg); font-weight: 600; text-align: right; word-break: break-all; }
    .alert { display: flex; align-items: flex-start; gap: 8px; margin: 0 0 16px; padding: 10px 12px; border: 1px solid var(--glass-border); border-radius: var(--radius-sm); background: var(--bad-soft); -webkit-backdrop-filter: blur(8px); backdrop-filter: blur(8px); color: var(--bad); font-size: 13px; box-shadow: var(--glass-rim); }
    .alert svg { width: 16px; height: 16px; flex: none; margin-top: 1px; fill: none; stroke: var(--bad); stroke-width: 2; }
    label { display: block; font-size: 13px; font-weight: 600; color: var(--fg-2); margin-bottom: 6px; }
    input[type="password"] { width: 100%; height: 44px; padding: 0 12px; font: inherit; color: var(--fg); background: var(--glass-bg-2); -webkit-backdrop-filter: blur(6px); backdrop-filter: blur(6px); border: 1px solid var(--glass-border); border-radius: var(--radius-sm); outline: none; box-shadow: var(--glass-rim); }
    input[type="password"]:focus { border-color: var(--accent); box-shadow: var(--glass-rim), 0 0 0 4px var(--ring); background: var(--glass-bg); }
    .hint { margin: 8px 0 0; font-size: 12.5px; color: var(--muted); }
    button { margin-top: 18px; width: 100%; height: 44px; font: inherit; font-weight: 600; color: var(--on-accent); background: var(--accent); border: none; border-radius: var(--radius-sm); box-shadow: inset 0 1px 0 rgba(255,255,255,.35), 0 8px 20px rgba(0,122,255,.3); cursor: pointer; transition: background .12s ease, transform .05s ease; }
    button:hover { background: var(--accent-press); }
    button:active { transform: scale(.99); }
    .foot { margin: 18px 0 0; font-size: 12px; color: var(--muted); text-align: center; }
"#;

fn redirect_origin(uri: &str) -> &str {
    let scheme_end = match uri.find("://") {
        Some(index) => index + 3,
        None => return uri,
    };
    let host = &uri[scheme_end..];
    let host_len = host.find('/').unwrap_or(host.len());
    &uri[..scheme_end + host_len]
}

pub fn authorization_approval_page(request: &AuthorizeRequest, rejected: bool) -> String {
    let mut state_field = String::new();
    if let Some(state) = &request.state {
        state_field = format!(
            r#"<input type="hidden" name="state" value="{}">"#,
            html_escape(state)
        );
    }
    let mut resource_field = String::new();
    if let Some(resource) = &request.resource {
        resource_field = format!(
            r#"<input type="hidden" name="resource" value="{}">"#,
            html_escape(resource)
        );
    }
    let hidden_fields = format!(
        r#"    <input type="hidden" name="response_type" value="{}">
    <input type="hidden" name="client_id" value="{}">
    <input type="hidden" name="redirect_uri" value="{}">
    <input type="hidden" name="code_challenge" value="{}">
    <input type="hidden" name="code_challenge_method" value="{}">
    {}
    {}"#,
        html_escape(&request.response_type),
        html_escape(&request.client_id),
        html_escape(&request.redirect_uri),
        html_escape(&request.code_challenge),
        html_escape(&request.code_challenge_method),
        state_field,
        resource_field,
    );
    let origin = html_escape(redirect_origin(&request.redirect_uri));
    let error_banner = if rejected {
        r#"<div class="alert" role="alert"><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M12 8v5"/><circle cx="12" cy="17" r=".6"/></svg><span>That approval token didn't match. Copy it again from the terminal where codex2gpt is running.</span></div>"#.to_string()
    } else {
        String::new()
    };
    let style = {
        let mut css = String::from("<style>");
        css.push_str(crate::ui::THEME_TOKENS);
        css.push_str(AUTH_PAGE_CSS);
        css.push_str("</style>");
        css
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<title>Authorize codex2gpt</title>
{style}
</head>
<body>
<main class="card">
  <div class="glyph" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="11" width="16" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/></svg></div>
  <h1>Authorize access</h1>
  <p class="lede">Enter the approval token shown where you started <strong>codex2gpt</strong>. This lets the app below connect to your local bridge.</p>
  <div class="requestor"><span class="k">Requesting app</span><span class="v">{origin}</span></div>
  {error}
  <form method="post" action="/oauth/authorize">
{hidden}
    <label for="approval_token">Approval token</label>
    <input id="approval_token" name="approval_token" type="password" autocomplete="off" autocapitalize="off" spellcheck="false" autofocus required>
    <p class="hint">You'll find this token in the terminal where codex2gpt is running.</p>
    <button type="submit">Approve</button>
  </form>
  <p class="foot">Local bridge · the token is checked on this device only.</p>
</main>
</body>
</html>"#,
        style = style,
        origin = origin,
        error = error_banner,
        hidden = hidden_fields,
    )
}

#[derive(Clone, Debug, Deserialize)]
pub struct TokenExchangeRequest {
    pub grant_type: String,
    pub client_id: String,
    pub code: String,
    pub redirect_uri: String,
    pub code_verifier: String,
    #[serde(default)]
    pub resource: Option<String>,
}

pub fn handle_token_exchange(
    state: &HttpState,
    request: TokenExchangeRequest,
) -> (StatusCode, Value) {
    if request.grant_type != "authorization_code" {
        return (
            StatusCode::BAD_REQUEST,
            json!({"error": "unsupported_grant_type"}),
        );
    }
    let access_token = new_url_token("token");
    let result = state
        .oauth_store
        .lock()
        .expect("oauth store mutex poisoned")
        .exchange_code(TokenRequest {
            client_id: &request.client_id,
            code: &request.code,
            redirect_uri: &request.redirect_uri,
            code_verifier: &request.code_verifier,
            access_token: &access_token,
            resource: request.resource.as_deref(),
        });

    match result {
        Ok(_) => (
            StatusCode::OK,
            json!({
                "access_token": access_token,
                "token_type": "Bearer",
                "expires_in": ACCESS_TOKEN_TTL_SECONDS
            }),
        ),
        Err(err) => (StatusCode::BAD_REQUEST, json!({"error": err})),
    }
}

async fn healthz_handler() -> &'static str {
    healthz()
}

async fn protected_resource_handler(State(state): State<HttpState>) -> axum::Json<Value> {
    axum::Json(protected_resource_metadata(
        &state.base_url,
        state.oauth_enabled(),
    ))
}

async fn authorization_server_handler(
    State(state): State<HttpState>,
) -> (StatusCode, axum::Json<Value>) {
    if !state.oauth_enabled() {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(json!({"error": "oauth_disabled"})),
        );
    }
    (
        StatusCode::OK,
        axum::Json(authorization_server_metadata(&state.base_url)),
    )
}

async fn oauth_register_post(
    State(state): State<HttpState>,
    axum::Json(request): axum::Json<ClientRegistrationRequest>,
) -> (StatusCode, axum::Json<Value>) {
    let (status, response) = handle_client_registration(&state, request);
    (status, axum::Json(response))
}

async fn oauth_authorize_get(
    State(state): State<HttpState>,
    Query(mut request): Query<AuthorizeRequest>,
) -> Response {
    request.approval_token.clear();
    authorize_response(state, request)
}

async fn oauth_authorize_post(
    State(state): State<HttpState>,
    Form(request): Form<AuthorizeRequest>,
) -> Response {
    authorize_response(state, request)
}

fn authorize_response(state: HttpState, request: AuthorizeRequest) -> Response {
    let redirect_uri = request.redirect_uri.clone();
    let request_state = request.state.clone();
    let rejected = !request.approval_token.is_empty();
    let approval_page = authorization_approval_page(&request, rejected);
    let (status, response) = handle_authorize_request(&state, request);
    if status == StatusCode::UNAUTHORIZED && response["error"] == "approval_required" {
        return (StatusCode::OK, Html(approval_page)).into_response();
    }
    if status != StatusCode::OK {
        return (status, axum::Json(response)).into_response();
    }
    let Some(code) = response["code"].as_str() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(json!({"error": "missing_authorization_code"})),
        )
            .into_response();
    };

    (
        StatusCode::FOUND,
        [(
            header::LOCATION,
            authorization_redirect_location(&redirect_uri, code, request_state.as_deref()),
        )],
    )
        .into_response()
}

async fn oauth_token_post(
    State(state): State<HttpState>,
    Form(request): Form<TokenExchangeRequest>,
) -> (StatusCode, axum::Json<Value>) {
    let (status, response) = handle_token_exchange(&state, request);
    (status, axum::Json(response))
}

async fn mcp_post(
    State(state): State<HttpState>,
    headers: HeaderMap,
    axum::Json(request): axum::Json<Value>,
) -> Response {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let (status, response) = handle_authenticated_http_json_rpc(&state, authorization, request);
    if status == StatusCode::UNAUTHORIZED {
        let challenge = www_authenticate_header(&state.base_url, state.oauth_enabled());
        return (
            status,
            [(header::WWW_AUTHENTICATE, challenge)],
            axum::Json(response),
        )
            .into_response();
    }

    (status, axum::Json(response)).into_response()
}

fn new_url_token(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .expect("failed to read random bytes for oauth token");
    format!("{prefix}-{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn html_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect(),
            '>' => "&gt;".chars().collect(),
            '"' => "&quot;".chars().collect(),
            '\'' => "&#39;".chars().collect(),
            _ => vec![ch],
        })
        .collect()
}
