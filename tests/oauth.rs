use codex2gpt::oauth::{
    ACCESS_TOKEN_TTL_SECONDS, AuthorizationCode, BearerGate, ClientRegistration, OAuthStore,
    TokenRecord, TokenRequest, is_allowed_redirect_uri, is_loopback_redirect_uri, verify_pkce_s256,
};

#[test]
fn pkce_s256_verifies_known_challenge() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

    assert!(verify_pkce_s256(verifier, challenge));
    assert!(!verify_pkce_s256("wrong", challenge));
}

#[test]
fn redirect_uri_allows_only_loopback_http() {
    assert!(is_loopback_redirect_uri("http://127.0.0.1:8910/callback"));
    assert!(is_loopback_redirect_uri("http://localhost:8910/callback"));
    assert!(!is_loopback_redirect_uri(
        "http://localhost:8910@evil.example/callback"
    ));
    assert!(!is_loopback_redirect_uri(
        "http://127.0.0.1:8910.evil.example/callback"
    ));
    assert!(!is_loopback_redirect_uri("https://example.com/callback"));
    assert!(!is_loopback_redirect_uri("http://evil.example/callback"));
}

#[test]
fn redirect_uri_allowlist_accepts_chatgpt_and_loopback_only() {
    assert!(is_allowed_redirect_uri(
        "https://chatgpt.com/oauth/callback"
    ));
    assert!(is_allowed_redirect_uri(
        "https://chat.openai.com/oauth/callback"
    ));
    assert!(is_allowed_redirect_uri(
        "https://chatgpt.com/connector/oauth/callback_123"
    ));
    assert!(is_allowed_redirect_uri(
        "https://chatgpt.com/connector_platform_oauth_redirect"
    ));
    assert!(is_allowed_redirect_uri("http://127.0.0.1:8910/callback"));
    assert!(!is_allowed_redirect_uri("https://chatgpt.com/"));
    assert!(!is_allowed_redirect_uri(
        "https://chatgpt.com/oauth/callback/extra"
    ));
    assert!(!is_allowed_redirect_uri(
        "https://chatgpt.com/connector/oauth/"
    ));
    assert!(!is_allowed_redirect_uri(
        "https://chatgpt.com/connector/oauth/callback_123/extra"
    ));
    assert!(!is_allowed_redirect_uri(
        "http://localhost:8910@evil.example/callback"
    ));
    assert!(!is_allowed_redirect_uri(
        "https://chat.openai.com/not-oauth/callback"
    ));
    assert!(!is_allowed_redirect_uri("https://evil.example/callback"));
}

#[test]
fn oauth_store_consumes_authorization_code_once() {
    let mut store = OAuthStore::default();
    store.insert_code(AuthorizationCode {
        code: "code-1".to_owned(),
        client_id: "client".to_owned(),
        redirect_uri: "http://127.0.0.1:8910/callback".to_owned(),
        code_challenge: "challenge".to_owned(),
        resource: None,
    });

    assert!(store.take_code("code-1").is_some());
    assert!(store.take_code("code-1").is_none());
}

#[test]
fn oauth_store_registers_clients_with_redirect_uris() {
    let mut store = OAuthStore::default();
    let client = store.register_client(ClientRegistration {
        redirect_uris: vec!["https://chatgpt.com/oauth/callback".to_owned()],
    });

    assert!(client.client_id.starts_with("client-"));
    assert_eq!(
        client.redirect_uris,
        vec!["https://chatgpt.com/oauth/callback".to_owned()]
    );
    assert!(store.redirect_uri_allowed(&client.client_id, "https://chatgpt.com/oauth/callback"));
    assert!(!store.redirect_uri_allowed(&client.client_id, "https://evil.example/callback"));
}

#[test]
fn oauth_store_exchanges_authorization_code_for_token_once() {
    let mut store = OAuthStore::default();
    let client = store.register_client(ClientRegistration {
        redirect_uris: vec!["https://chatgpt.com/oauth/callback".to_owned()],
    });
    store.insert_code(AuthorizationCode {
        code: "code-1".to_owned(),
        client_id: client.client_id.clone(),
        redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
        code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
        resource: None,
    });

    let earliest_expiry = unix_millis() + u128::from(ACCESS_TOKEN_TTL_SECONDS) * 1000;
    let token = store
        .exchange_code(TokenRequest {
            client_id: &client.client_id,
            code: "code-1",
            redirect_uri: "https://chatgpt.com/oauth/callback",
            code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            access_token: "oauth-token",
            resource: None,
        })
        .unwrap();
    let latest_expiry = unix_millis() + u128::from(ACCESS_TOKEN_TTL_SECONDS) * 1000;

    assert_eq!(token.client_id, client.client_id);
    assert!(token.expires_at_unix_millis >= earliest_expiry);
    assert!(token.expires_at_unix_millis <= latest_expiry);
    assert!(store.validate_token("oauth-token"));
    assert!(
        store
            .exchange_code(TokenRequest {
                client_id: &token.client_id,
                code: "code-1",
                redirect_uri: "https://chatgpt.com/oauth/callback",
                code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                access_token: "another-token",
                resource: None,
            })
            .is_err()
    );
}

#[test]
fn oauth_store_rejects_code_exchange_with_wrong_pkce_or_redirect() {
    let mut store = OAuthStore::default();
    let client = store.register_client(ClientRegistration {
        redirect_uris: vec!["https://chatgpt.com/oauth/callback".to_owned()],
    });
    store.insert_code(AuthorizationCode {
        code: "code-1".to_owned(),
        client_id: client.client_id.clone(),
        redirect_uri: "https://chatgpt.com/oauth/callback".to_owned(),
        code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM".to_owned(),
        resource: None,
    });

    assert!(
        store
            .exchange_code(TokenRequest {
                client_id: &client.client_id,
                code: "code-1",
                redirect_uri: "https://wrong.example/callback",
                code_verifier: "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                access_token: "oauth-token",
                resource: None,
            })
            .is_err()
    );

    assert!(
        store
            .exchange_code(TokenRequest {
                client_id: &client.client_id,
                code: "code-1",
                redirect_uri: "https://chatgpt.com/oauth/callback",
                code_verifier: "wrong",
                access_token: "oauth-token",
                resource: None,
            })
            .is_err()
    );
}

#[test]
fn oauth_store_hashes_bearer_tokens() {
    let mut store = OAuthStore::default();
    store.insert_token(
        "plain-token",
        TokenRecord {
            client_id: "client".to_owned(),
            expires_at_unix_millis: u128::MAX,
        },
    );

    assert!(store.validate_token("plain-token"));
    assert!(!store.validate_token("other-token"));
    assert!(
        !store
            .debug_token_hashes()
            .iter()
            .any(|value| value == "plain-token")
    );
}

#[test]
fn oauth_store_rejects_expired_bearer_tokens() {
    let mut store = OAuthStore::default();
    store.insert_token(
        "plain-token",
        TokenRecord {
            client_id: "client".to_owned(),
            expires_at_unix_millis: 0,
        },
    );

    assert!(!store.validate_token("plain-token"));
}

#[test]
fn bearer_gate_accepts_only_matching_authorization_header() {
    let gate = BearerGate::new("local-secret");

    assert!(gate.allows(Some("Bearer local-secret")));
    assert!(gate.allows(Some("bearer local-secret")));
    assert!(!gate.allows(None));
    assert!(!gate.allows(Some("Bearer wrong")));
    assert!(!gate.allows(Some("Basic local-secret")));
}

#[test]
fn bearer_gate_does_not_expose_plain_token_in_debug() {
    let gate = BearerGate::new("plain-token");

    assert!(!format!("{gate:?}").contains("plain-token"));
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
