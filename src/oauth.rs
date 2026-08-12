use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ACCESS_TOKEN_TTL_SECONDS: u64 = 3600;
const ACCESS_TOKEN_TTL_MILLIS: u128 = ACCESS_TOKEN_TTL_SECONDS as u128 * 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizationCode {
    pub code: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub resource: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenRecord {
    pub client_id: String,
    pub expires_at_unix_millis: u128,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientRegistration {
    pub redirect_uris: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisteredClient {
    pub client_id: String,
    pub redirect_uris: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenRequest<'a> {
    pub client_id: &'a str,
    pub code: &'a str,
    pub redirect_uri: &'a str,
    pub code_verifier: &'a str,
    pub access_token: &'a str,
    pub resource: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BearerGate {
    token_hash: String,
}

impl BearerGate {
    pub fn new(token: &str) -> Self {
        Self {
            token_hash: hash_secret(token),
        }
    }

    pub fn allows(&self, authorization_header: Option<&str>) -> bool {
        let Some(header) = authorization_header else {
            return false;
        };
        let Some(token) = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
        else {
            return false;
        };

        self.token_hash == hash_secret(token)
    }
}

#[derive(Debug, Default)]
pub struct OAuthStore {
    clients: HashMap<String, RegisteredClient>,
    codes: HashMap<String, AuthorizationCode>,
    tokens_by_hash: HashMap<String, TokenRecord>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedOAuthClients {
    clients: Vec<RegisteredClient>,
}

impl OAuthStore {
    pub fn load_clients(path: &Path) -> io::Result<Self> {
        let raw = match fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        let persisted: PersistedOAuthClients = serde_json::from_str(&raw)
            .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source))?;
        let mut store = Self::default();
        for client in persisted.clients {
            store.clients.insert(client.client_id.clone(), client);
        }
        Ok(store)
    }

    pub fn save_clients(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut clients = self.clients.values().cloned().collect::<Vec<_>>();
        clients.sort_by(|left, right| left.client_id.cmp(&right.client_id));
        let raw = serde_json::to_string_pretty(&PersistedOAuthClients { clients })
            .map_err(io::Error::other)?;
        let mut tmp_path = path.to_path_buf();
        tmp_path.set_extension(format!("tmp-{}", std::process::id()));
        fs::write(&tmp_path, raw)?;
        fs::rename(tmp_path, path)
    }

    pub fn register_client(&mut self, registration: ClientRegistration) -> RegisteredClient {
        let client = RegisteredClient {
            client_id: new_oauth_id("client"),
            redirect_uris: registration.redirect_uris,
        };
        self.clients
            .insert(client.client_id.clone(), client.clone());
        client
    }

    pub fn ensure_client_with_redirect_uri(&mut self, client_id: &str, redirect_uri: &str) -> bool {
        if client_id.is_empty() || !is_allowed_redirect_uri(redirect_uri) {
            return false;
        }
        if self.clients.contains_key(client_id) {
            return self.redirect_uri_allowed(client_id, redirect_uri);
        }
        self.clients.insert(
            client_id.to_owned(),
            RegisteredClient {
                client_id: client_id.to_owned(),
                redirect_uris: vec![redirect_uri.to_owned()],
            },
        );
        true
    }

    pub fn redirect_uri_allowed(&self, client_id: &str, redirect_uri: &str) -> bool {
        self.clients
            .get(client_id)
            .is_some_and(|client| client.redirect_uris.iter().any(|uri| uri == redirect_uri))
    }

    pub fn insert_code(&mut self, code: AuthorizationCode) {
        self.codes.insert(code.code.clone(), code);
    }

    pub fn take_code(&mut self, code: &str) -> Option<AuthorizationCode> {
        self.codes.remove(code)
    }

    pub fn insert_token(&mut self, token: &str, record: TokenRecord) {
        self.tokens_by_hash.insert(hash_secret(token), record);
    }

    pub fn exchange_code(&mut self, request: TokenRequest<'_>) -> Result<TokenRecord, String> {
        let Some(code) = self.codes.get(request.code).cloned() else {
            return Err("unknown authorization code".to_owned());
        };
        if code.client_id != request.client_id {
            return Err("authorization code client mismatch".to_owned());
        }
        if code.redirect_uri != request.redirect_uri {
            return Err("authorization code redirect mismatch".to_owned());
        }
        if !self.redirect_uri_allowed(request.client_id, request.redirect_uri) {
            return Err("redirect uri is not registered".to_owned());
        }
        if code.resource.as_deref() != request.resource {
            return Err("authorization code resource mismatch".to_owned());
        }
        if !verify_pkce_s256(request.code_verifier, &code.code_challenge) {
            return Err("pkce verification failed".to_owned());
        }

        self.codes.remove(request.code);
        let record = TokenRecord {
            client_id: request.client_id.to_owned(),
            expires_at_unix_millis: now_unix_millis().saturating_add(ACCESS_TOKEN_TTL_MILLIS),
        };
        self.insert_token(request.access_token, record.clone());
        Ok(record)
    }

    pub fn validate_token(&self, token: &str) -> bool {
        self.tokens_by_hash
            .get(&hash_secret(token))
            .is_some_and(|record| record.expires_at_unix_millis > now_unix_millis())
    }

    pub fn debug_token_hashes(&self) -> Vec<String> {
        self.tokens_by_hash.keys().cloned().collect()
    }
}

pub fn verify_pkce_s256(verifier: &str, challenge: &str) -> bool {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest) == challenge
}

pub fn is_loopback_redirect_uri(uri: &str) -> bool {
    let Some(rest) = uri.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let Some((host, port)) = authority.rsplit_once(':') else {
        return false;
    };
    !authority.contains('@')
        && matches!(host, "127.0.0.1" | "localhost")
        && !port.is_empty()
        && port.chars().all(|ch| ch.is_ascii_digit())
}

pub fn is_allowed_redirect_uri(uri: &str) -> bool {
    is_loopback_redirect_uri(uri)
        || uri == "https://chatgpt.com/oauth/callback"
        || uri == "https://chat.openai.com/oauth/callback"
        || is_chatgpt_connector_redirect_uri(uri)
        || uri == "https://chatgpt.com/connector_platform_oauth_redirect"
}

fn is_chatgpt_connector_redirect_uri(uri: &str) -> bool {
    let Some(callback_id) = uri.strip_prefix("https://chatgpt.com/connector/oauth/") else {
        return false;
    };
    !callback_id.is_empty() && !callback_id.contains(['/', '?', '#'])
}

fn hash_secret(secret: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(secret.as_bytes()))
}

fn now_unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before UNIX epoch")
        .as_millis()
}

fn new_oauth_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before UNIX epoch")
        .as_nanos();
    format!("{prefix}-{}-{nanos}", std::process::id())
}
