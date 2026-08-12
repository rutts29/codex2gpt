use std::fs;
use std::path::Path;

use codex2gpt::audit::{AuditEvent, redact_for_log};
use codex2gpt::config::{AppConfig, ToolSurface};
use codex2gpt::paths::resolve_workspace_path;

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "codex2gpt-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn config_loads_workspace_defaults_from_json() {
    let root = unique_temp_dir("config-root");
    let state = unique_temp_dir("config-state");
    let config_path = state.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "listen_addr": "127.0.0.1:8787",
              "state_dir": "{}",
              "allowed_workspaces": [
                {{"id": "bridge", "path": "{}", "allow_write": true}}
              ]
            }}"#,
            state.display(),
            root.display()
        ),
    )
    .unwrap();

    let config = AppConfig::load_from_file(&config_path).unwrap();

    assert_eq!(config.listen_addr, "127.0.0.1:8787");
    assert_eq!(config.codex_binary, "codex");
    assert_eq!(config.git_binary, "git");
    assert_eq!(config.rg_binary, "rg");
    assert_eq!(config.max_read_bytes, 64 * 1024);
    assert_eq!(config.max_search_results, 100);
    assert_eq!(config.tool_surface, ToolSurface::Full);
    assert_eq!(config.workspace("bridge").unwrap().path, root);
}

#[test]
fn config_loads_advisor_tool_surface() {
    let root = unique_temp_dir("advisor-root");
    let state = unique_temp_dir("advisor-state");
    let config_path = state.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "tool_surface": "advisor",
              "allowed_workspaces": [
                {{"id": "bridge", "path": "{}"}}
              ]
            }}"#,
            state.display(),
            root.display()
        ),
    )
    .unwrap();

    let config = AppConfig::load_from_file(&config_path).unwrap();

    assert_eq!(config.tool_surface, ToolSurface::Advisor);
}

#[test]
fn config_rejects_duplicate_workspace_ids() {
    let root = unique_temp_dir("duplicate-root");
    let state = unique_temp_dir("duplicate-state");
    let config_path = state.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "allowed_workspaces": [
                {{"id": "same", "path": "{}"}},
                {{"id": "same", "path": "{}"}}
              ]
            }}"#,
            state.display(),
            root.display(),
            root.display()
        ),
    )
    .unwrap();

    let err = AppConfig::load_from_file(&config_path).unwrap_err();

    assert!(err.to_string().contains("duplicate workspace id"));
}

#[test]
fn config_rejects_non_loopback_listen_addr() {
    let root = unique_temp_dir("listen-root");
    let state = unique_temp_dir("listen-state");
    let config_path = state.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "listen_addr": "0.0.0.0:8787",
              "state_dir": "{}",
              "allowed_workspaces": [
                {{"id": "bridge", "path": "{}"}}
              ]
            }}"#,
            state.display(),
            root.display()
        ),
    )
    .unwrap();

    let err = AppConfig::load_from_file(&config_path).unwrap_err();

    assert!(err.to_string().contains("listen_addr"));
}

#[test]
fn config_rejects_widget_domain_that_is_not_https_origin() {
    for widget_domain in [
        "https://codex2gpt.example.test/path",
        "https://bad host",
        "https://codex2gpt.example.test:notaport",
    ] {
        let root = unique_temp_dir("widget-domain-root");
        let state = unique_temp_dir("widget-domain-state");
        let config_path = state.join("config.json");
        fs::write(
            &config_path,
            format!(
                r#"{{
                  "state_dir": "{}",
                  "widget_domain": "{}",
                  "allowed_workspaces": [
                    {{"id": "bridge", "path": "{}"}}
                  ]
                }}"#,
                state.display(),
                widget_domain,
                root.display()
            ),
        )
        .unwrap();

        let err = AppConfig::load_from_file(&config_path).unwrap_err();

        assert!(err.to_string().contains("widget_domain"));
    }
}

#[test]
fn resolve_workspace_path_rejects_parent_escape() {
    let root = unique_temp_dir("escape-root");

    let err = resolve_workspace_path(&root, Path::new("../outside.txt")).unwrap_err();

    assert!(err.to_string().contains("escapes workspace"));
}

#[test]
fn resolve_workspace_path_rejects_symlink_escape() {
    #[cfg(unix)]
    {
        let root = unique_temp_dir("symlink-root");
        let outside = unique_temp_dir("symlink-outside");
        let link = root.join("outside-link");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let err = resolve_workspace_path(&root, Path::new("outside-link/file.txt")).unwrap_err();

        assert!(err.to_string().contains("symlink"));
    }
}

#[test]
fn resolve_workspace_path_rejects_internal_symlinks() {
    #[cfg(unix)]
    {
        let root = unique_temp_dir("internal-symlink-root");
        fs::write(root.join("real.txt"), "hello").unwrap();
        std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt")).unwrap();

        let err = resolve_workspace_path(&root, Path::new("link.txt")).unwrap_err();

        assert!(err.to_string().contains("symlink"));
    }
}

#[test]
fn redact_for_log_hides_token_like_values() {
    let input = r#"Authorization:Bearer sk-test token=abc123 api_key="secret" OPENAI_API_KEY=sk-live normal text"#;

    let redacted = redact_for_log(input);

    assert!(!redacted.contains("sk-test"));
    assert!(!redacted.contains("abc123"));
    assert!(!redacted.contains("secret"));
    assert!(!redacted.contains("sk-live"));
    assert!(redacted.contains("normal text"));
}

#[test]
fn audit_event_serializes_without_plain_secret() {
    let event = AuditEvent::new("tool.call", "Authorization: Bearer secret-value");

    let encoded = serde_json::to_string(&event).unwrap();

    assert!(encoded.contains("tool.call"));
    assert!(!encoded.contains("secret-value"));
}
