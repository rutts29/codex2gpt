use std::fs;
use std::path::Path;

use codex2gpt::config::AppConfig;
use codex2gpt::mcp::{AppState, handle_json_rpc};
use codex2gpt::runs::RunStore;
use serde_json::{Value, json};

fn state_with_workspace() -> AppState {
    state_with_rg_body(
        r#"{"type":"match","data":{"path":{"text":"notes.txt"},"lines":{"text":"hello from workspace\n"},"line_number":1}}"#,
    )
}

fn state_with_rg_body(rg_body: &str) -> AppState {
    let root = unique_temp_dir("mcp-root");
    fs::write(root.join("README.md"), "# Bridge\n").unwrap();
    fs::write(root.join("notes.txt"), "hello from workspace\n").unwrap();
    state_with_rg_body_for_root(&root, rg_body)
}

fn state_with_rg_body_for_root(root: &Path, rg_body: &str) -> AppState {
    state_with_rg_body_for_root_and_surface(root, rg_body, "full")
}

fn state_with_rg_body_for_root_and_surface(
    root: &Path,
    rg_body: &str,
    tool_surface: &str,
) -> AppState {
    let state_dir = unique_temp_dir("mcp-state");
    let fake_rg = state_dir.join("fake-rg");
    write_fake_rg(&fake_rg, rg_body);
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "rg_binary": "{}",
              "tool_surface": "{}",
              "allowed_workspaces": [
                {{"id": "bridge", "path": "{}", "allow_write": false}}
              ]
            }}"#,
            state_dir.display(),
            fake_rg.display(),
            tool_surface,
            root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

#[test]
fn tools_list_advisor_surface_exposes_only_pro_safe_read_tools() {
    let root = unique_temp_dir("mcp-advisor-root");
    fs::write(root.join("oauth.rs"), "OAuth token and PKCE flow\n").unwrap();
    let state = state_with_rg_body_for_root_and_surface(
        &root,
        r#"{"type":"match","data":{"path":{"text":"oauth.rs"},"lines":{"text":"OAuth token and PKCE flow\n"},"line_number":1}}"#,
        "advisor",
    );

    let response = handle_json_rpc(
        &state,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    );

    let names = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["list_workspaces", "search", "fetch", "check_connection"]
    );
    for tool in response["result"]["tools"].as_array().unwrap() {
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["annotations"]["openWorldHint"], false);
    }
}

fn state_with_git_body_for_root(root: &Path, git_body: &str) -> AppState {
    state_with_git_body_for_root_and_write(root, git_body, false)
}

fn state_with_git_body_for_root_and_write(
    root: &Path,
    git_body: &str,
    allow_write: bool,
) -> AppState {
    let state_dir = unique_temp_dir("mcp-git-state");
    let fake_git = state_dir.join("fake-git");
    write_fake_git(&fake_git, git_body);
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "git_binary": "{}",
              "allowed_workspaces": [
                {{"id": "bridge", "path": "{}", "allow_write": {}}}
              ]
            }}"#,
            state_dir.display(),
            fake_git.display(),
            root.display(),
            allow_write
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

fn state_with_rg_body_and_read_limit(rg_body: &str, max_read_bytes: usize) -> AppState {
    let root = unique_temp_dir("mcp-root");
    fs::write(root.join("README.md"), "# Bridge\n").unwrap();
    fs::write(root.join("notes.txt"), "hello from workspace\n").unwrap();
    let state_dir = unique_temp_dir("mcp-state");
    let fake_rg = state_dir.join("fake-rg");
    write_fake_rg(&fake_rg, rg_body);
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "rg_binary": "{}",
              "max_read_bytes": {},
              "allowed_workspaces": [
                {{"id": "bridge", "path": "{}", "allow_write": false}}
              ]
            }}"#,
            state_dir.display(),
            fake_rg.display(),
            max_read_bytes,
            root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

fn state_with_widget_domain(widget_domain: &str) -> AppState {
    let root = unique_temp_dir("mcp-widget-root");
    let state_dir = unique_temp_dir("mcp-widget-state");
    let config_path = state_dir.join("config.json");
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
            state_dir.display(),
            widget_domain,
            root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

#[cfg(unix)]
fn write_fake_rg(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, format!("#!/bin/sh\ncat <<'EOF'\n{body}\nEOF\n")).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn write_fake_git(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, format!("#!/bin/sh\ncat <<'EOF'\n{body}\nEOF\n")).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn state_with_fake_appserver() -> AppState {
    let root = unique_temp_dir("mcp-appserver-root");
    let state_dir = unique_temp_dir("mcp-appserver-state");
    let fake_codex = state_dir.join("fake-codex");
    write_fake_appserver_codex(&fake_codex);
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "codex_binary": "{}",
              "allowed_workspaces": [
                {{"id": "repo", "path": "{}", "allow_write": true}}
              ]
            }}"#,
            state_dir.display(),
            fake_codex.display(),
            root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

#[cfg(unix)]
fn state_with_two_fake_appserver_workspaces() -> AppState {
    let repo_root = unique_temp_dir("mcp-appserver-repo-root");
    let other_root = unique_temp_dir("mcp-appserver-other-root");
    let state_dir = unique_temp_dir("mcp-appserver-two-state");
    let fake_codex = state_dir.join("fake-codex");
    write_fake_appserver_codex(&fake_codex);
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "codex_binary": "{}",
              "allowed_workspaces": [
                {{"id": "repo", "path": "{}", "allow_write": true}},
                {{"id": "other", "path": "{}", "allow_write": true}}
              ]
            }}"#,
            state_dir.display(),
            fake_codex.display(),
            repo_root.display(),
            other_root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

#[cfg(unix)]
fn state_with_fake_appserver_and_git() -> AppState {
    let root = unique_temp_dir("mcp-appserver-git-root");
    state_with_fake_appserver_and_git_root(&root)
}

#[cfg(unix)]
fn state_with_fake_appserver_and_git_root(root: &Path) -> AppState {
    let state_dir = unique_temp_dir("mcp-appserver-git-state");
    let fake_codex = state_dir.join("fake-codex");
    let fake_git = state_dir.join("fake-git");
    write_fake_appserver_codex(&fake_codex);
    write_fake_git(
        &fake_git,
        r#"worktree /tmp/placeholder
HEAD abc123
branch refs/heads/main
"#,
    );
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "codex_binary": "{}",
              "git_binary": "{}",
              "allowed_workspaces": [
                {{"id": "repo", "path": "{}", "allow_write": true}}
              ]
            }}"#,
            state_dir.display(),
            fake_codex.display(),
            fake_git.display(),
            root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
}

#[cfg(unix)]
fn write_fake_appserver_codex(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(
        path,
        r#"#!/usr/bin/env python3
import json
import os
import sys

def send(obj):
    print(json.dumps(obj), flush=True)

def log(message):
    with open(sys.argv[0] + ".requests.jsonl", "a") as out:
        out.write(json.dumps(message) + "\n")

for raw in sys.stdin:
    if not raw.strip():
        continue
    message = json.loads(raw)
    log(message)
    method = message.get("method")
    params = message.get("params", {})
    if method == "initialize":
        send({"id": message.get("id"), "result": {"serverInfo": {"name": "fake"}}})
    elif method == "initialized":
        pass
    elif method == "thread/list":
        parent = os.path.dirname(os.path.realpath(params.get("cwd")))
        managed = os.path.join(parent, ".codex2gpt-worktrees", "repo", "feature-a")
        symlink_escape = os.path.join(parent, ".codex2gpt-worktrees", "repo", "linked-outside")
        escaped = os.path.join(managed, "..", "..", "outside")
        threads = [{"id": "thr_repo", "cwd": params.get("cwd")}, {"id": "thr_worktree", "cwd": managed}, {"id": "thr_escape", "cwd": escaped}, {"id": "thr_other", "cwd": "/tmp/other-repo"}, {"id": "thr_unknown"}]
        if os.path.lexists(symlink_escape):
            threads.append({"id": "thr_symlink_escape", "cwd": symlink_escape})
        send({"id": message.get("id"), "result": {"threads": threads, "nextCursor": None}})
    elif method == "thread/start":
        send({"id": message.get("id"), "result": {"thread": {"id": "thr_started", "cwd": params.get("cwd")}}})
        send({"method": "thread/started", "params": {"threadId": "thr_started", "cwd": params.get("cwd")}})
    elif method == "thread/resume":
        cwd = "/tmp/other-repo" if params.get("threadId") == "thr_other" else params.get("cwd")
        send({"id": message.get("id"), "result": {"thread": {"id": params.get("threadId"), "cwd": cwd}}})
    elif method == "thread/fork":
        send({"id": message.get("id"), "result": {"thread": {"id": "thr_forked", "source": params.get("threadId")}}})
    elif method == "thread/read":
        send({"id": message.get("id"), "result": {"thread": {"id": params.get("threadId"), "turns": [{"id": "turn_1"}]}}})
    elif method == "turn/start":
        send({"id": message.get("id"), "result": {"turn": {"id": "turn_2", "threadId": params.get("threadId"), "status": "queued"}}})
        send({"method": "turn/started", "params": {"threadId": params.get("threadId"), "turnId": "turn_2"}})
        send({"method": "item/completed", "params": {"threadId": params.get("threadId"), "item": {"type": "agent_message", "text": "done"}}})
        send({"method": "item/completed", "params": {"threadId": params.get("threadId"), "item": {"phase": "final_answer", "text": "final done"}}})
        send({"method": "item/completed", "params": {"threadId": params.get("threadId"), "item": {"type": "command_execution", "command": "cargo test", "status": "success"}}})
        send({"method": "item/completed", "params": {"threadId": params.get("threadId"), "branch": "codex2gpt/feature-a", "item": {"type": "file_change", "path": "src/lib.rs", "diffSummary": "src/lib.rs modified"}}})
        send({"method": "thread/tokenUsage/updated", "params": {"threadId": params.get("threadId"), "tokenUsage": {"total": {"totalTokens": 123}}}})
        send({"method": "thread/status/changed", "params": {"threadId": params.get("threadId"), "status": {"type": "idle"}}})
        send({"method": "turn/completed", "params": {"threadId": params.get("threadId"), "turn": {"id": "turn_2", "status": "completed"}, "usage": {"totalTokens": 123}}})
    elif method == "turn/steer":
        send({"id": message.get("id"), "result": {"turnId": params.get("turnId", "turn_2")}})
    elif method == "turn/interrupt":
        send({"id": message.get("id"), "result": {"interrupted": True}})
    elif method == "thread/backgroundTerminals/list":
        send({"id": message.get("id"), "result": {"terminals": [{"processId": 42, "command": "npm run dev"}]}})
    elif method == "thread/backgroundTerminals/clean":
        send({"id": message.get("id"), "result": {"cleaned": True}})
    elif method == "thread/backgroundTerminals/terminate":
        send({"id": message.get("id"), "result": {"terminated": True, "processId": params.get("processId")}})
    elif method == "thread/rollback":
        send({"id": message.get("id"), "result": {"thread": {"id": params.get("threadId"), "rolledBackTurns": params.get("turns")}}})
    elif method == "thread/unarchive":
        send({"id": message.get("id"), "result": {"thread": {"id": params.get("threadId"), "archived": False}}})
    elif method == "model/list":
        send({"id": message.get("id"), "result": {"models": [{"id": "gpt-5"}]}})
    elif method == "review/start":
        send({"id": message.get("id"), "result": {"review": {"id": "review_1", "threadId": params.get("threadId")}}})
        send({"id": 700, "method": "execApproval", "params": {"threadId": "thr_started", "command": ["cargo", "test"]}})
        send({"id": "req-string", "method": "execApproval", "params": {"threadId": "thr_started", "command": ["cargo", "test"]}})
    elif method == "config/get":
        send({"id": message.get("id"), "result": {"config": {"sandbox": "workspace-write"}}})
    elif method == "mcp/list":
        send({"id": message.get("id"), "result": {"servers": []}})
    elif method == "features/list":
        send({"id": message.get("id"), "result": {"features": {"webSearch": True}}})
    else:
        send({"id": message.get("id"), "result": {}})
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

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
fn initialize_returns_server_info() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );

    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "codex2gpt");
    assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
}

#[test]
fn tools_list_returns_explicit_hints() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    );

    let tools = response["result"]["tools"].as_array().unwrap();
    for tool in tools {
        assert!(tool.get("inputSchema").is_some());
        assert!(tool.get("outputSchema").is_some());
        assert_eq!(
            tool["securitySchemes"],
            json!([{"type": "oauth2", "scopes": []}])
        );
        assert_eq!(tool["_meta"]["securitySchemes"], tool["securitySchemes"]);
        assert!(tool["annotations"].get("readOnlyHint").is_some());
        assert!(tool["annotations"].get("destructiveHint").is_some());
        assert!(tool["annotations"].get("openWorldHint").is_some());
    }

    let list_workspaces = tools
        .iter()
        .find(|tool| tool["name"] == "list_workspaces")
        .unwrap();

    assert_eq!(list_workspaces["annotations"]["readOnlyHint"], true);
    assert_eq!(list_workspaces["annotations"]["destructiveHint"], false);
    assert_eq!(list_workspaces["annotations"]["openWorldHint"], false);

    let search_context = tools
        .iter()
        .find(|tool| tool["name"] == "search_context")
        .unwrap();

    assert_eq!(search_context["annotations"]["readOnlyHint"], true);
    assert_eq!(search_context["annotations"]["destructiveHint"], false);
    assert_eq!(search_context["annotations"]["openWorldHint"], false);

    let search = tools.iter().find(|tool| tool["name"] == "search").unwrap();

    assert_eq!(
        search["inputSchema"]["properties"],
        json!({"workspace": {"type": "string"}, "query": {"type": "string"}})
    );
    assert_eq!(
        search["inputSchema"]["required"],
        json!(["workspace", "query"])
    );
    assert_eq!(search["annotations"]["readOnlyHint"], true);
    assert_eq!(search["annotations"]["destructiveHint"], false);
    assert_eq!(search["annotations"]["openWorldHint"], false);

    let fetch = tools.iter().find(|tool| tool["name"] == "fetch").unwrap();

    assert_eq!(
        fetch["inputSchema"]["properties"],
        json!({"id": {"type": "string"}, "url": {"type": "string"}})
    );
    assert_eq!(
        fetch["inputSchema"]["anyOf"],
        json!([{"required": ["id"]}, {"required": ["url"]}])
    );
    assert_eq!(fetch["annotations"]["readOnlyHint"], true);
    assert_eq!(fetch["annotations"]["destructiveHint"], false);
    assert_eq!(fetch["annotations"]["openWorldHint"], false);

    let smoke = tools
        .iter()
        .find(|tool| tool["name"] == "check_connection")
        .unwrap();
    assert_eq!(
        smoke["inputSchema"]["properties"],
        json!({"workspace": {"type": "string"}})
    );
    assert_eq!(smoke["inputSchema"]["required"], json!(["workspace"]));
    assert_eq!(smoke["annotations"]["readOnlyHint"], true);
    assert_eq!(smoke["annotations"]["destructiveHint"], false);
    assert_eq!(smoke["annotations"]["openWorldHint"], false);

    assert!(
        !tools
            .iter()
            .any(|tool| tool["name"] == "run_readonly_smoke_test")
    );

    let start_readonly = tools
        .iter()
        .find(|tool| tool["name"] == "start_readonly_codex_thread")
        .unwrap();

    assert_eq!(start_readonly["annotations"]["readOnlyHint"], true);
    assert_eq!(start_readonly["annotations"]["destructiveHint"], false);
    assert_eq!(start_readonly["annotations"]["openWorldHint"], false);

    for name in [
        "start_codex_thread",
        "send_codex_turn",
        "set_run_options",
        "run_in_worktree",
    ] {
        let tool = tools.iter().find(|tool| tool["name"] == name).unwrap();
        assert_eq!(tool["annotations"]["openWorldHint"], true);
    }

    let export_bundle = tools
        .iter()
        .find(|tool| tool["name"] == "export_result_bundle")
        .unwrap();
    assert_eq!(
        export_bundle["outputSchema"]["properties"]["thread_id"],
        json!({"type": "string"})
    );
    assert_eq!(
        export_bundle["outputSchema"]["properties"]["events"]["type"],
        "array"
    );
    assert_eq!(
        export_bundle["_meta"]["ui"]["resourceUri"],
        "ui://codex2gpt/result-bundle-v2.html"
    );
    assert_eq!(
        export_bundle["_meta"]["openai/outputTemplate"],
        "ui://codex2gpt/result-bundle-v2.html"
    );
    assert_eq!(
        export_bundle["_meta"]["ui"]["visibility"],
        json!(["model", "app"])
    );
    assert!(export_bundle["_meta"]["openai/widgetAccessible"].is_null());

    let approvals = tools
        .iter()
        .find(|tool| tool["name"] == "approval_bridge")
        .unwrap();
    assert_eq!(
        approvals["outputSchema"]["properties"]["pending"]["type"],
        "array"
    );
    assert_eq!(
        approvals["_meta"]["ui"]["resourceUri"],
        "ui://codex2gpt/approvals-v2.html"
    );
    assert_eq!(
        approvals["_meta"]["ui"]["visibility"],
        json!(["model", "app"])
    );
    assert!(approvals["_meta"]["openai/widgetAccessible"].is_null());

    let list_worktrees = tools
        .iter()
        .find(|tool| tool["name"] == "list_worktrees")
        .unwrap();

    assert_eq!(list_worktrees["annotations"]["readOnlyHint"], true);
    assert_eq!(list_worktrees["annotations"]["destructiveHint"], false);
    assert_eq!(list_worktrees["annotations"]["openWorldHint"], false);

    let create_worktree = tools
        .iter()
        .find(|tool| tool["name"] == "create_worktree")
        .unwrap();

    assert_eq!(create_worktree["annotations"]["readOnlyHint"], false);
    assert_eq!(create_worktree["annotations"]["destructiveHint"], false);
    assert_eq!(create_worktree["annotations"]["openWorldHint"], false);

    let remove_worktree = tools
        .iter()
        .find(|tool| tool["name"] == "remove_worktree")
        .unwrap();

    assert_eq!(remove_worktree["annotations"]["readOnlyHint"], false);
    assert_eq!(remove_worktree["annotations"]["destructiveHint"], true);
    assert_eq!(remove_worktree["annotations"]["openWorldHint"], false);

    let resume_thread = tools
        .iter()
        .find(|tool| tool["name"] == "resume_codex_thread")
        .unwrap();
    assert_ne!(
        resume_thread["outputSchema"],
        json!({"type": "object", "additionalProperties": true})
    );
    assert_eq!(
        resume_thread["outputSchema"]["properties"]["thread"]["type"],
        "object"
    );
    assert_eq!(
        resume_thread["inputSchema"]["properties"]["worktree"]["pattern"],
        "^[A-Za-z0-9_-]{1,80}$"
    );

    let fork_thread = tools
        .iter()
        .find(|tool| tool["name"] == "fork_codex_thread")
        .unwrap();
    assert_eq!(
        fork_thread["inputSchema"]["properties"]["worktree"]["pattern"],
        "^[A-Za-z0-9_-]{1,80}$"
    );

    for tool in tools.iter().filter(|tool| {
        tool["name"].as_str().is_some_and(|name| {
            name.contains("codex")
                || name.contains("thread")
                || name.contains("terminal")
                || matches!(
                    name,
                    "list_models"
                        | "set_run_options"
                        | "approval_bridge"
                        | "export_result_bundle"
                        | "run_in_worktree"
                        | "list_hooks_skills_mcp"
                )
        })
    }) {
        assert_ne!(
            tool["outputSchema"],
            json!({"type": "object", "additionalProperties": true})
        );
        assert!(tool["outputSchema"].get("properties").is_some());
    }

    let interrupt = tools
        .iter()
        .find(|tool| tool["name"] == "interrupt_codex_turn")
        .unwrap();
    assert_eq!(
        interrupt["outputSchema"]["properties"]["interrupted"],
        json!({"type": "boolean"})
    );

    let clean_terminals = tools
        .iter()
        .find(|tool| tool["name"] == "clean_background_terminals")
        .unwrap();
    assert_eq!(
        clean_terminals["outputSchema"]["properties"]["cleaned"],
        json!({"type": "boolean"})
    );

    let terminate_terminal = tools
        .iter()
        .find(|tool| tool["name"] == "terminate_background_terminal")
        .unwrap();
    assert_eq!(
        terminate_terminal["outputSchema"]["properties"]["terminated"],
        json!({"type": "boolean"})
    );

    let rollback = tools
        .iter()
        .find(|tool| tool["name"] == "rollback_thread")
        .unwrap();
    assert_eq!(
        rollback["outputSchema"]["properties"]["thread"]["type"],
        "object"
    );
}

#[test]
fn tools_list_omits_legacy_exec_tools() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    );

    let tools = response["result"]["tools"].as_array().unwrap();

    assert!(!tools.iter().any(|tool| tool["name"] == "start_codex_task"));
    assert!(!tools.iter().any(|tool| tool["name"] == "get_codex_task"));
}

#[test]
fn resources_list_exposes_chatgpt_widget_templates() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "id": 21, "method": "resources/list"}),
    );

    let resources = response["result"]["resources"].as_array().unwrap();
    assert!(resources.iter().any(|resource| {
        resource["uri"] == "ui://codex2gpt/result-bundle-v2.html"
            && resource["mimeType"] == "text/html;profile=mcp-app"
    }));
    assert!(resources.iter().any(|resource| {
        resource["uri"] == "ui://codex2gpt/approvals-v2.html"
            && resource["mimeType"] == "text/html;profile=mcp-app"
    }));
}

#[test]
fn resources_list_advisor_surface_omits_full_mode_widgets() {
    let root = unique_temp_dir("mcp-advisor-resources-root");
    fs::write(root.join("oauth.rs"), "OAuth token and PKCE flow\n").unwrap();
    let state = state_with_rg_body_for_root_and_surface(
        &root,
        r#"{"type":"match","data":{"path":{"text":"oauth.rs"},"lines":{"text":"OAuth token and PKCE flow\n"},"line_number":1}}"#,
        "advisor",
    );

    let response = handle_json_rpc(
        &state,
        json!({"jsonrpc": "2.0", "id": 21_1, "method": "resources/list"}),
    );

    assert_eq!(
        response["result"]["resources"].as_array().unwrap(),
        &Vec::<Value>::new()
    );
}

#[test]
fn resources_read_returns_widget_html_with_csp_metadata() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "resources/read",
            "params": {"uri": "ui://codex2gpt/result-bundle-v2.html"}
        }),
    );

    let content = &response["result"]["contents"][0];
    assert_eq!(content["uri"], "ui://codex2gpt/result-bundle-v2.html");
    assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
    assert!(content["text"].as_str().unwrap().contains("window.openai"));
    assert_eq!(content["_meta"]["ui"]["csp"]["connectDomains"], json!([]));
    assert_eq!(content["_meta"]["ui"]["csp"]["resourceDomains"], json!([]));
    assert!(content["_meta"]["openai/widgetDescription"].is_string());
}

#[test]
fn resources_read_includes_configured_widget_domain_metadata() {
    let response = handle_json_rpc(
        &state_with_widget_domain("https://codex2gpt.example.test"),
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "resources/read",
            "params": {"uri": "ui://codex2gpt/result-bundle-v2.html"}
        }),
    );

    let content = &response["result"]["contents"][0];
    assert_eq!(
        content["_meta"]["ui"]["domain"],
        "https://codex2gpt.example.test"
    );
    assert_eq!(
        content["_meta"]["openai/widgetDomain"],
        "https://codex2gpt.example.test"
    );
}

#[test]
fn tools_call_list_workspaces_returns_configured_workspaces() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "list_workspaces", "arguments": {}}
        }),
    );

    let structured = &response["result"]["structuredContent"];

    assert_eq!(structured["workspaces"][0]["id"], "bridge");
    assert_eq!(structured["workspaces"][0]["allowWrite"], false);
}

#[test]
#[cfg(unix)]
fn tools_call_list_worktrees_returns_git_worktrees() {
    let root = unique_temp_dir("mcp-worktree-root");
    let response = handle_json_rpc(
        &state_with_git_body_for_root(
            &root,
            &format!(
                r#"worktree {}
HEAD abc123
branch refs/heads/main
"#,
                root.display()
            ),
        ),
        json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "list_worktrees",
                "arguments": {"workspace": "bridge"}
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];

    assert_eq!(structured["workspace_id"], "bridge");
    assert_eq!(
        structured["worktrees"][0]["path"].as_str().unwrap(),
        root.file_name().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(structured["worktrees"][0]["branch"], "main");
    assert_eq!(structured["worktrees"][0]["commit"], "abc123");
}

#[test]
#[cfg(unix)]
fn tools_call_create_worktree_returns_created_worktree() {
    let root = unique_temp_dir("mcp-create-worktree-root");
    let state = state_with_git_body_for_root_and_write(&root, "", true);
    let response = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "tools/call",
            "params": {
                "name": "create_worktree",
                "arguments": {
                    "workspace": "bridge",
                    "name": "feature-a",
                    "base": "main"
                }
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];

    assert_eq!(structured["workspace_id"], "bridge");
    assert_eq!(structured["name"], "feature-a");
    assert_eq!(structured["branch"], "codex2gpt/feature-a");
    assert_eq!(structured["base"], "main");
    assert_eq!(structured["path"], ".codex2gpt-worktrees/bridge/feature-a");

    let audit_log = fs::read_to_string(state.config.state_dir.join("audit.jsonl")).unwrap();
    assert!(audit_log.contains(r#""kind":"create_worktree""#));
    assert!(audit_log.contains("workspace=bridge name=feature-a base=main"));
}

#[test]
#[cfg(unix)]
fn tools_call_create_worktree_rejects_read_only_workspace() {
    let root = unique_temp_dir("mcp-create-readonly-root");
    let response = handle_json_rpc(
        &state_with_git_body_for_root(&root, ""),
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "tools/call",
            "params": {
                "name": "create_worktree",
                "arguments": {
                    "workspace": "bridge",
                    "name": "feature-a",
                    "base": "main"
                }
            }
        }),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not allow writes")
    );
}

#[test]
#[cfg(unix)]
fn tools_call_remove_worktree_returns_removed_worktree() {
    let root = unique_temp_dir("mcp-remove-worktree-root");
    let response = handle_json_rpc(
        &state_with_git_body_for_root_and_write(&root, "", true),
        json!({
            "jsonrpc": "2.0",
            "id": 23,
            "method": "tools/call",
            "params": {
                "name": "remove_worktree",
                "arguments": {
                    "workspace": "bridge",
                    "name": "feature-a"
                }
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];

    assert_eq!(structured["workspace_id"], "bridge");
    assert_eq!(structured["name"], "feature-a");
    assert_eq!(structured["path"], ".codex2gpt-worktrees/bridge/feature-a");
}

#[test]
#[cfg(unix)]
fn tools_call_remove_worktree_rejects_read_only_workspace() {
    let root = unique_temp_dir("mcp-remove-readonly-root");
    let response = handle_json_rpc(
        &state_with_git_body_for_root(&root, ""),
        json!({
            "jsonrpc": "2.0",
            "id": 24,
            "method": "tools/call",
            "params": {
                "name": "remove_worktree",
                "arguments": {
                    "workspace": "bridge",
                    "name": "feature-a"
                }
            }
        }),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not allow writes")
    );
}

#[test]
fn tools_call_repo_brief_returns_workspace_summary() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "tools/call",
            "params": {
                "name": "repo_brief",
                "arguments": {"workspace": "bridge"}
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];

    assert_eq!(structured["workspace_id"], "bridge");
    assert!(
        structured["entries"]
            .as_array()
            .unwrap()
            .contains(&json!("README.md"))
    );
}

#[test]
fn tools_call_read_context_returns_file_text() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "tools/call",
            "params": {
                "name": "read_context",
                "arguments": {"workspace": "bridge", "path": "notes.txt"}
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];

    assert_eq!(structured["path"], "notes.txt");
    assert_eq!(structured["text"], "hello from workspace\n");
}

#[test]
fn tools_call_search_context_returns_matches() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "tools/call",
            "params": {
                "name": "search_context",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];

    assert_eq!(structured["query"], "hello");
    assert_eq!(structured["matches"][0]["path"], "notes.txt");
    assert_eq!(structured["matches"][0]["line"], 1);
    assert_eq!(structured["matches"][0]["text"], "hello from workspace");
    assert_eq!(structured["truncated"], false);
}

#[test]
fn tools_call_search_returns_standard_compatibility_payload() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 33,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];
    let content_payload: Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();

    assert_eq!(structured, &content_payload);
    assert_eq!(structured["results"][0]["title"], "notes.txt");
    assert!(
        structured["results"][0]["id"]
            .as_str()
            .unwrap()
            .starts_with("bridge/")
    );
    assert_eq!(
        structured["results"][0]["url"].as_str().unwrap(),
        format!(
            "https://codex2gpt.local/document/{}",
            structured["results"][0]["id"].as_str().unwrap()
        )
    );
}

#[test]
fn tools_call_run_readonly_smoke_test_returns_fixed_oauth_context() {
    let root = unique_temp_dir("mcp-smoke-root");
    fs::write(root.join("oauth.rs"), "OAuth token and PKCE flow\n").unwrap();
    fs::write(root.join("other.rs"), "nothing here\n").unwrap();
    let state = state_with_rg_body_for_root(
        &root,
        r#"{"type":"match","data":{"path":{"text":"oauth.rs"},"lines":{"text":"OAuth token and PKCE flow\n"},"line_number":1}}"#,
    );

    let response = call_tool(
        &state,
        33_1,
        "run_readonly_smoke_test",
        json!({"workspace": "bridge"}),
    );

    let structured = &response["result"]["structuredContent"];
    assert_eq!(structured["workspace"]["id"], "bridge");
    assert_eq!(structured["query"], "OAuth");
    assert_eq!(structured["search"]["results"][0]["title"], "oauth.rs");
    assert_eq!(structured["fetched"]["title"], "oauth.rs");
    assert_eq!(structured["fetched"]["text"], "OAuth token and PKCE flow\n");
}

#[test]
fn tools_call_check_connection_returns_fixed_oauth_context() {
    let root = unique_temp_dir("mcp-check-root");
    fs::write(root.join("oauth.rs"), "OAuth token and PKCE flow\n").unwrap();
    let state = state_with_rg_body_for_root(
        &root,
        r#"{"type":"match","data":{"path":{"text":"oauth.rs"},"lines":{"text":"OAuth token and PKCE flow\n"},"line_number":1}}"#,
    );

    let response = call_tool(
        &state,
        33_2,
        "check_connection",
        json!({"workspace": "bridge"}),
    );

    let structured = &response["result"]["structuredContent"];
    assert_eq!(structured["workspace"]["id"], "bridge");
    assert_eq!(structured["fetched"]["title"], "oauth.rs");
    assert_eq!(structured["fetched"]["text"], "OAuth token and PKCE flow\n");
}

#[test]
fn tools_call_search_returns_one_result_per_file() {
    let state = state_with_rg_body(
        r#"{"type":"match","data":{"path":{"text":"notes.txt"},"lines":{"text":"hello one\n"},"line_number":1}}
{"type":"match","data":{"path":{"text":"notes.txt"},"lines":{"text":"hello two\n"},"line_number":2}}"#,
    );

    let response = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 36,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );

    let results = response["result"]["structuredContent"]["results"]
        .as_array()
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["title"], "notes.txt");
}

#[test]
fn tools_call_fetch_returns_standard_compatibility_payload() {
    let state = state_with_workspace();
    let search = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 34,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );
    let document_id = search["result"]["structuredContent"]["results"][0]["id"]
        .as_str()
        .unwrap();

    let response = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 35,
            "method": "tools/call",
            "params": {
                "name": "fetch",
                "arguments": {"id": document_id}
            }
        }),
    );

    let structured = &response["result"]["structuredContent"];
    let content_payload: Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();

    assert_eq!(structured, &content_payload);
    assert_eq!(structured["id"], document_id);
    assert_eq!(structured["title"], "notes.txt");
    assert_eq!(structured["text"], "hello from workspace\n");
    assert_eq!(structured["metadata"]["workspace"], "bridge");
    assert_eq!(structured["metadata"]["path"], "notes.txt");
    assert_eq!(structured["metadata"]["truncated"], false);
}

#[test]
fn tools_call_fetch_rejects_forged_document_ids() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 37,
            "method": "tools/call",
            "params": {
                "name": "fetch",
                "arguments": {"id": "v1:not-valid-base64"}
            }
        }),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalid document id")
    );
}

#[test]
fn tools_call_search_and_fetch_do_not_expose_absolute_paths() {
    let state = state_with_workspace();
    let search = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 38,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );
    let document_id = search["result"]["structuredContent"]["results"][0]["id"]
        .as_str()
        .unwrap();
    let fetch = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 39,
            "method": "tools/call",
            "params": {
                "name": "fetch",
                "arguments": {"id": document_id}
            }
        }),
    );

    let visible = format!(
        "{}{}",
        search["result"]["structuredContent"], fetch["result"]["structuredContent"]
    );

    assert!(!visible.contains("/Users/"));
    assert!(!visible.contains(std::env::temp_dir().to_string_lossy().as_ref()));
}

#[test]
fn tools_call_fetch_accepts_returned_search_url() {
    let state = state_with_workspace();
    let search = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 38_1,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );
    let document_url = search["result"]["structuredContent"]["results"][0]["url"]
        .as_str()
        .unwrap();
    let fetch = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 38_2,
            "method": "tools/call",
            "params": {
                "name": "fetch",
                "arguments": {"id": document_url}
            }
        }),
    );

    assert_eq!(fetch["result"]["structuredContent"]["title"], "notes.txt");
    assert_eq!(
        fetch["result"]["structuredContent"]["text"],
        "hello from workspace\n"
    );
}

#[test]
fn tools_call_fetch_accepts_returned_https_search_url() {
    let search = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 45,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );
    let document_url = search["result"]["structuredContent"]["results"][0]["url"]
        .as_str()
        .unwrap();

    assert!(document_url.starts_with("https://codex2gpt.local/document/bridge/"));

    let fetch = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 46,
            "method": "tools/call",
            "params": {
                "name": "fetch",
                "arguments": {"id": document_url}
            }
        }),
    );

    assert_eq!(fetch["result"]["structuredContent"]["title"], "notes.txt");
}

#[test]
fn tools_call_fetch_accepts_url_argument_from_search_result() {
    let search = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 45_1,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );
    let document_url = search["result"]["structuredContent"]["results"][0]["url"]
        .as_str()
        .unwrap();

    let fetch = handle_json_rpc(
        &state_with_workspace(),
        json!({
            "jsonrpc": "2.0",
            "id": 46_1,
            "method": "tools/call",
            "params": {
                "name": "fetch",
                "arguments": {"url": document_url}
            }
        }),
    );

    assert_eq!(fetch["result"]["structuredContent"]["title"], "notes.txt");
}

#[test]
fn tools_call_fetch_reports_truncation_metadata() {
    let state = state_with_rg_body_and_read_limit(
        r#"{"type":"match","data":{"path":{"text":"notes.txt"},"lines":{"text":"hello from workspace\n"},"line_number":1}}"#,
        5,
    );
    let search = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 45,
            "method": "tools/call",
            "params": {
                "name": "search",
                "arguments": {"workspace": "bridge", "query": "hello"}
            }
        }),
    );
    let document_id = search["result"]["structuredContent"]["results"][0]["id"]
        .as_str()
        .unwrap();
    let fetch = handle_json_rpc(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": 46,
            "method": "tools/call",
            "params": {
                "name": "fetch",
                "arguments": {"id": document_id}
            }
        }),
    );

    assert_eq!(fetch["result"]["structuredContent"]["text"], "hello");
    assert_eq!(
        fetch["result"]["structuredContent"]["metadata"]["truncated"],
        true
    );
}

#[test]
#[cfg(unix)]
fn tools_call_appserver_thread_and_turn_tools() {
    let state = state_with_fake_appserver();

    let list = call_tool(
        &state,
        50,
        "list_codex_threads",
        json!({"workspace": "repo"}),
    );
    assert_eq!(
        list["result"]["structuredContent"]["threads"][0]["id"],
        "thr_repo"
    );
    let threads = list["result"]["structuredContent"]["threads"]
        .as_array()
        .unwrap();
    assert_eq!(threads.len(), 2);
    assert!(threads.iter().any(|thread| thread["id"] == "thr_worktree"));
    assert!(!threads.iter().any(|thread| thread["id"] == "thr_escape"));

    let start = call_tool(
        &state,
        51,
        "start_codex_thread",
        json!({"workspace": "repo", "prompt": "plan this", "sandbox": "workspace-write"}),
    );
    assert_eq!(
        start["result"]["structuredContent"]["thread"]["id"],
        "thr_started"
    );
    let run_id = start["result"]["structuredContent"]["run"]["run_id"]
        .as_str()
        .unwrap();
    let linked = RunStore::new(state.config.state_dir.join("runs"))
        .load_status(run_id)
        .unwrap();
    assert_eq!(linked.thread_id.as_deref(), Some("thr_started"));

    let resume = call_tool(
        &state,
        52,
        "resume_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert_eq!(
        resume["result"]["structuredContent"]["thread"]["id"],
        "thr_started"
    );

    let fork = call_tool(
        &state,
        53,
        "fork_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert_eq!(
        fork["result"]["structuredContent"]["thread"]["id"],
        "thr_forked"
    );
    let read_fork = call_tool(
        &state,
        53_1,
        "read_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_forked"}),
    );
    assert_eq!(
        read_fork["result"]["structuredContent"]["thread"]["turns"][0]["id"],
        "turn_1"
    );

    let read = call_tool(
        &state,
        54,
        "read_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert_eq!(
        read["result"]["structuredContent"]["thread"]["turns"][0]["id"],
        "turn_1"
    );

    let turn = call_tool(
        &state,
        55,
        "send_codex_turn",
        json!({"workspace": "repo", "thread_id": "thr_started", "prompt": "continue"}),
    );
    assert_eq!(turn["result"]["structuredContent"]["turn"]["id"], "turn_2");

    let steered = call_tool(
        &state,
        55_1,
        "steer_codex_turn",
        json!({"workspace": "repo", "thread_id": "thr_started", "turn_id": "turn_2", "prompt": "actually focus tests"}),
    );
    assert_eq!(steered["result"]["structuredContent"]["turnId"], "turn_2");

    let interrupted = call_tool(
        &state,
        56,
        "interrupt_codex_turn",
        json!({"workspace": "repo", "thread_id": "thr_started", "turn_id": "turn_2"}),
    );
    assert_eq!(
        interrupted["result"]["structuredContent"]["interrupted"],
        true
    );

    let events = call_tool(
        &state,
        57,
        "stream_codex_events",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert!(
        events["result"]["structuredContent"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["method"] == "turn/started")
    );
    assert_eq!(
        events["result"]["structuredContent"]["summary"]["final_message"],
        "final done"
    );
    assert_eq!(
        events["result"]["structuredContent"]["summary"]["commands_run"][0],
        "cargo test"
    );
    assert_eq!(
        events["result"]["structuredContent"]["summary"]["changed_files"][0],
        "src/lib.rs"
    );
}

#[test]
#[cfg(unix)]
fn list_codex_threads_rejects_managed_worktree_symlink_escape() {
    use std::os::unix::fs::symlink;

    let state = state_with_fake_appserver();
    let workspace = state.config.workspace("repo").unwrap();
    let root = workspace.path.canonicalize().unwrap();
    let parent = root.parent().unwrap();
    let managed_root = parent.join(".codex2gpt-worktrees").join("repo");
    fs::create_dir_all(&managed_root).unwrap();
    let outside = unique_temp_dir("mcp-appserver-symlink-outside");
    let link_path = managed_root.join("linked-outside");
    if fs::symlink_metadata(&link_path).is_ok() {
        fs::remove_file(&link_path).unwrap();
    }
    symlink(&outside, link_path).unwrap();

    let list = call_tool(
        &state,
        50_1,
        "list_codex_threads",
        json!({"workspace": "repo"}),
    );
    let threads = list["result"]["structuredContent"]["threads"]
        .as_array()
        .unwrap();

    assert!(
        !threads
            .iter()
            .any(|thread| thread["id"] == "thr_symlink_escape")
    );
}

#[test]
#[cfg(unix)]
fn tools_call_appserver_capability_review_and_approval_tools() {
    let state = state_with_fake_appserver();
    call_tool(
        &state,
        57_1,
        "start_codex_thread",
        json!({"workspace": "repo", "prompt": "prepare review"}),
    );

    let models = call_tool(&state, 58, "list_models", json!({}));
    assert_eq!(
        models["result"]["structuredContent"]["models"][0]["id"],
        "gpt-5"
    );

    let capabilities = call_tool(
        &state,
        59,
        "list_hooks_skills_mcp",
        json!({"workspace": "repo"}),
    );
    assert_eq!(
        capabilities["result"]["structuredContent"]["config"]["sandbox"],
        "workspace-write"
    );
    assert!(capabilities["result"]["structuredContent"]["mcp"]["servers"].is_array());
    assert_eq!(
        capabilities["result"]["structuredContent"]["features"]["features"]["webSearch"],
        true
    );

    let review = call_tool(
        &state,
        60,
        "review_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert_eq!(
        review["result"]["structuredContent"]["review"]["id"],
        "review_1"
    );

    let pending = wait_for_pending_approvals(&state, "repo", 2);
    assert_eq!(
        pending["result"]["structuredContent"]["pending"][0]["method"],
        "execApproval"
    );
    let denied = call_tool(
        &state,
        63,
        "approval_bridge",
        json!({"workspace": "repo", "request_id": 700, "decision": "deny", "reason": "not approved"}),
    );
    assert_eq!(denied["result"]["structuredContent"]["decision"], "deny");

    let denied_string = call_tool(
        &state,
        63_2,
        "approval_bridge",
        json!({"workspace": "repo", "request_id": "req-string", "decision": "deny", "reason": "not approved"}),
    );
    assert_eq!(
        denied_string["result"]["structuredContent"]["request_id"],
        "req-string"
    );
}

#[test]
#[cfg(unix)]
fn tools_call_appserver_background_lifecycle_and_bundle_tools() {
    let state = state_with_fake_appserver();
    call_tool(
        &state,
        64_1,
        "start_codex_thread",
        json!({"workspace": "repo", "prompt": "prepare background tools"}),
    );
    call_tool(
        &state,
        65,
        "send_codex_turn",
        json!({"workspace": "repo", "thread_id": "thr_started", "prompt": "finish"}),
    );

    let terminals = call_tool(
        &state,
        66,
        "list_background_terminals",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert_eq!(
        terminals["result"]["structuredContent"]["terminals"][0]["processId"],
        42
    );

    let terminated = call_tool(
        &state,
        67,
        "terminate_background_terminal",
        json!({"workspace": "repo", "thread_id": "thr_started", "process_id": 42}),
    );
    assert_eq!(terminated["result"]["structuredContent"]["processId"], 42);

    let cleaned = call_tool(
        &state,
        68,
        "clean_background_terminals",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert_eq!(cleaned["result"]["structuredContent"]["cleaned"], true);

    let rolled_back = call_tool(
        &state,
        69,
        "rollback_thread",
        json!({"workspace": "repo", "thread_id": "thr_started", "turns": 1}),
    );
    assert_eq!(
        rolled_back["result"]["structuredContent"]["thread"]["rolledBackTurns"],
        1
    );

    let unarchived = call_tool(
        &state,
        70,
        "unarchive_thread",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    assert_eq!(
        unarchived["result"]["structuredContent"]["thread"]["archived"],
        false
    );

    let bundle = call_tool(
        &state,
        71,
        "export_result_bundle",
        json!({"workspace": "repo", "thread_id": "thr_started"}),
    );
    let structured = &bundle["result"]["structuredContent"];
    assert_eq!(structured["thread_id"], "thr_started");
    assert_eq!(structured["final_message"], "final done");
    assert_eq!(structured["status"], "completed");
    assert_eq!(structured["commands_run"], json!(["cargo test"]));
    assert_eq!(structured["tests_run"], json!(["cargo test"]));
    assert_eq!(structured["changed_files"], json!(["src/lib.rs"]));
    assert_eq!(structured["branch"], "codex2gpt/feature-a");
    assert_eq!(structured["diff_summary"], json!(["src/lib.rs modified"]));
    assert_eq!(structured["token_usage"]["totalTokens"], 123);
}

#[test]
#[cfg(unix)]
fn tools_call_start_codex_thread_rejects_write_for_read_only_workspace() {
    let root = unique_temp_dir("mcp-appserver-readonly-root");
    let state_dir = unique_temp_dir("mcp-appserver-readonly-state");
    let fake_codex = state_dir.join("fake-codex");
    write_fake_appserver_codex(&fake_codex);
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "codex_binary": "{}",
              "allowed_workspaces": [
                {{"id": "repo", "path": "{}", "allow_write": false}}
              ]
            }}"#,
            state_dir.display(),
            fake_codex.display(),
            root.display()
        ),
    )
    .unwrap();
    let state = AppState::new(AppConfig::load_from_file(&config_path).unwrap());

    let response = call_tool(
        &state,
        64,
        "start_codex_thread",
        json!({"workspace": "repo", "prompt": "edit", "sandbox": "workspace-write"}),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("workspace does not allow writes")
    );
}

#[test]
#[cfg(unix)]
fn tools_call_thread_scoped_tools_reject_unbound_thread_ids() {
    let state = state_with_fake_appserver();

    let response = call_tool(
        &state,
        72,
        "send_codex_turn",
        json!({"workspace": "repo", "thread_id": "thr_unknown", "prompt": "continue"}),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("thread is not linked")
    );
}

#[test]
#[cfg(unix)]
fn resume_codex_thread_rejects_threads_outside_workspace() {
    let state = state_with_fake_appserver();

    let response = call_tool(
        &state,
        72_1,
        "resume_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_other"}),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("workspace")
    );
}

#[test]
#[cfg(unix)]
fn approval_bridge_requires_workspace_scope() {
    let state = state_with_fake_appserver();
    let response = call_tool(&state, 72_11, "approval_bridge", json!({}));

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("workspace")
    );
}

#[test]
#[cfg(unix)]
fn set_run_options_are_applied_to_future_threads() {
    let state = state_with_fake_appserver();

    let saved = call_tool(
        &state,
        72_2,
        "set_run_options",
        json!({
            "workspace": "repo",
            "options": {
                "model": "gpt-5",
                "reasoning_effort": "high",
                "web_search": true
            }
        }),
    );
    assert!(saved.get("error").is_none());

    let started = call_tool(
        &state,
        72_3,
        "start_codex_thread",
        json!({"workspace": "repo", "prompt": "use saved options"}),
    );
    assert!(started.get("error").is_none());

    let requests = fake_appserver_requests(&state);
    let thread_start = requests
        .iter()
        .find(|request| request["method"] == "thread/start")
        .unwrap();
    let params = &thread_start["params"];
    assert_eq!(params["model"], "gpt-5");
    assert_eq!(params["reasoning_effort"], "high");
    assert!(params.get("sandbox").is_none());
    assert_eq!(params["web_search"], true);
}

#[test]
#[cfg(unix)]
fn set_run_options_requires_workspace() {
    let state = state_with_fake_appserver();

    let response = call_tool(
        &state,
        72_21,
        "set_run_options",
        json!({"options": {"model": "gpt-5"}}),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("workspace")
    );
}

#[test]
#[cfg(unix)]
fn set_run_options_are_workspace_scoped() {
    let state = state_with_two_fake_appserver_workspaces();

    let saved = call_tool(
        &state,
        72_22,
        "set_run_options",
        json!({"workspace": "repo", "options": {"model": "gpt-5"}}),
    );
    assert!(saved.get("error").is_none());

    let started = call_tool(
        &state,
        72_23,
        "start_codex_thread",
        json!({"workspace": "other", "prompt": "do not inherit repo options"}),
    );
    assert!(started.get("error").is_none());

    let requests = fake_appserver_requests(&state);
    let thread_start = requests
        .iter()
        .filter(|request| request["method"] == "thread/start")
        .next_back()
        .unwrap();

    assert!(thread_start["params"].get("model").is_none());
}

#[test]
#[cfg(unix)]
fn tools_call_start_readonly_codex_thread_forces_read_only_sandbox() {
    let state = state_with_fake_appserver();

    let started = call_tool(
        &state,
        72_24,
        "start_readonly_codex_thread",
        json!({
            "workspace": "repo",
            "prompt": "summarize the repo",
            "sandbox": "workspace-write",
            "options": {
                "model": "gpt-5"
            }
        }),
    );

    assert_eq!(
        started["result"]["structuredContent"]["thread"]["id"],
        "thr_started"
    );
    let requests = fake_appserver_requests(&state);
    let thread_start = requests
        .iter()
        .filter(|request| request["method"] == "thread/start")
        .next_back()
        .unwrap();
    let params = &thread_start["params"];
    assert_eq!(params["sandbox"], "read-only");
    assert_eq!(params["model"], "gpt-5");
    assert!(params.get("web_search").is_none());
}

#[test]
#[cfg(unix)]
fn tools_call_start_readonly_codex_thread_rejects_web_search() {
    let state = state_with_fake_appserver();

    let response = call_tool(
        &state,
        72_25,
        "start_readonly_codex_thread",
        json!({
            "workspace": "repo",
            "prompt": "summarize the repo",
            "options": {
                "web_search": true
            }
        }),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("web_search")
    );
}

#[test]
#[cfg(unix)]
fn set_run_options_rejects_policy_escalation_keys() {
    let state = state_with_fake_appserver();

    for key in ["sandbox", "approval_policy", "network_access"] {
        let mut options = serde_json::Map::new();
        options.insert(key.to_owned(), json!("workspace-write"));
        let response = call_tool(
            &state,
            72_31,
            "set_run_options",
            json!({"workspace": "repo", "options": Value::Object(options)}),
        );
        assert_eq!(response["error"]["code"], -32602);
    }
}

#[test]
#[cfg(unix)]
fn run_in_worktree_links_started_thread_for_follow_up_turns() {
    let state = state_with_fake_appserver_and_git();

    let started = call_tool(
        &state,
        72_4,
        "run_in_worktree",
        json!({"workspace": "repo", "name": "feature-a", "base": "main", "prompt": "work here"}),
    );
    assert_eq!(
        started["result"]["structuredContent"]["thread"]["thread"]["id"],
        "thr_started"
    );

    let turn = call_tool(
        &state,
        72_5,
        "send_codex_turn",
        json!({"workspace": "repo", "thread_id": "thr_started", "prompt": "continue"}),
    );
    assert!(turn.get("error").is_none(), "{turn}");
}

#[test]
#[cfg(unix)]
fn resume_codex_thread_can_target_managed_worktree() {
    let state = state_with_fake_appserver_and_git();

    let started = call_tool(
        &state,
        72_6,
        "run_in_worktree",
        json!({"workspace": "repo", "name": "feature-a", "base": "main", "prompt": "work here"}),
    );
    assert!(started.get("error").is_none(), "{started}");

    let resumed = call_tool(
        &state,
        72_7,
        "resume_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_started", "worktree": "feature-a"}),
    );
    assert!(resumed.get("error").is_none(), "{resumed}");

    let requests = fake_appserver_requests(&state);
    let resume = requests
        .iter()
        .rev()
        .find(|request| request["method"] == "thread/resume")
        .unwrap();
    let cwd = resume["params"]["cwd"].as_str().unwrap();
    assert!(cwd.ends_with(".codex2gpt-worktrees/repo/feature-a"));
}

#[test]
#[cfg(unix)]
fn fork_codex_thread_can_target_managed_worktree() {
    let state = state_with_fake_appserver_and_git();

    let started = call_tool(
        &state,
        72_8,
        "run_in_worktree",
        json!({"workspace": "repo", "name": "feature-a", "base": "main", "prompt": "work here"}),
    );
    assert!(started.get("error").is_none(), "{started}");

    let forked = call_tool(
        &state,
        72_9,
        "fork_codex_thread",
        json!({"workspace": "repo", "thread_id": "thr_started", "worktree": "feature-a"}),
    );
    assert!(forked.get("error").is_none(), "{forked}");

    let requests = fake_appserver_requests(&state);
    let fork = requests
        .iter()
        .rev()
        .find(|request| request["method"] == "thread/fork")
        .unwrap();
    let cwd = fork["params"]["cwd"].as_str().unwrap();
    assert!(cwd.ends_with(".codex2gpt-worktrees/repo/feature-a"));
}

#[test]
#[cfg(unix)]
fn run_in_worktree_uses_canonical_workspace_parent_for_cwd() {
    use std::os::unix::fs::symlink;

    let real_root = unique_temp_dir("mcp-appserver-real-root");
    let link_parent = unique_temp_dir("mcp-appserver-link-parent");
    let link_root = link_parent.join("repo-link");
    symlink(&real_root, &link_root).unwrap();
    let state = state_with_fake_appserver_and_git_root(&link_root);

    let started = call_tool(
        &state,
        72_6,
        "run_in_worktree",
        json!({"workspace": "repo", "name": "feature-a", "base": "main", "prompt": "work here"}),
    );
    assert!(started.get("error").is_none(), "{started}");

    let requests = fake_appserver_requests(&state);
    let thread_start = requests
        .iter()
        .find(|request| request["method"] == "thread/start")
        .unwrap();
    let cwd = thread_start["params"]["cwd"].as_str().unwrap();
    let canonical_parent = real_root
        .canonicalize()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    assert!(cwd.starts_with(canonical_parent.to_str().unwrap()));
    assert!(!cwd.starts_with(link_parent.to_str().unwrap()));
}

#[test]
#[cfg(unix)]
fn tools_call_rejects_extra_read_dirs_outside_workspace() {
    let state = state_with_fake_appserver();

    let response = call_tool(
        &state,
        73,
        "start_codex_thread",
        json!({
            "workspace": "repo",
            "prompt": "inspect",
            "options": {
                "extra_read_dirs": ["/etc"]
            }
        }),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("extra_read_dirs")
    );
}

#[test]
#[cfg(unix)]
fn tools_call_rejects_images_outside_workspace() {
    let state = state_with_fake_appserver();

    let response = call_tool(
        &state,
        73_1,
        "set_run_options",
        json!({
            "workspace": "repo",
            "options": {
                "images": ["/etc/passwd"]
            }
        }),
    );

    assert_eq!(response["error"]["code"], -32602);
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("images")
    );
}

#[cfg(unix)]
fn fake_appserver_requests(state: &AppState) -> Vec<Value> {
    let path = Path::new(&state.config.codex_binary).with_file_name("fake-codex.requests.jsonl");
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[cfg(unix)]
fn call_tool(state: &AppState, id: i64, name: &str, arguments: Value) -> Value {
    handle_json_rpc(
        state,
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        }),
    )
}

#[cfg(unix)]
fn wait_for_pending_approvals(state: &AppState, workspace: &str, expected_count: usize) -> Value {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        let pending = call_tool(
            state,
            62,
            "approval_bridge",
            json!({"workspace": workspace}),
        );
        if pending["result"]["structuredContent"]["pending"]
            .as_array()
            .is_some_and(|items| items.len() >= expected_count)
        {
            return pending;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    call_tool(
        state,
        62,
        "approval_bridge",
        json!({"workspace": workspace}),
    )
}

#[test]
fn unknown_method_returns_json_rpc_error() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "missing/method"}),
    );

    assert_eq!(response["id"], 4);
    assert_eq!(response["error"]["code"], -32601);
}

#[test]
fn malformed_request_returns_invalid_request_error() {
    let response = handle_json_rpc(&state_with_workspace(), Value::String("nope".to_owned()));

    assert_eq!(response["id"], Value::Null);
    assert_eq!(response["error"]["code"], -32600);
}

#[test]
fn requests_must_use_json_rpc_2_0() {
    let missing_version = handle_json_rpc(
        &state_with_workspace(),
        json!({"id": 4, "method": "tools/list"}),
    );
    let wrong_version = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "1.0", "id": 5, "method": "tools/list"}),
    );
    let missing_id = handle_json_rpc(&state_with_workspace(), json!({"method": "tools/list"}));

    assert_eq!(missing_version["id"], 4);
    assert_eq!(missing_version["error"]["code"], -32600);
    assert_eq!(wrong_version["id"], 5);
    assert_eq!(wrong_version["error"]["code"], -32600);
    assert_eq!(missing_id["id"], Value::Null);
    assert_eq!(missing_id["error"]["code"], -32600);
}

#[test]
fn notifications_do_not_return_json_rpc_responses() {
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    );

    assert_eq!(response, Value::Null);
}
