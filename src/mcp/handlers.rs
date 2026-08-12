use serde_json::{Value, json};
use std::fs;

use crate::audit::{AuditEvent, append_audit_event};
use crate::config::AppConfig;
use crate::context::{read_context, repo_brief, search_context};
use crate::runs::{RunState, RunStore};
use crate::worktrees::{create_worktree, list_worktrees, remove_worktree};

use super::{AppState, error, ok};
use super::{compat, policy};

pub(crate) fn list_workspaces(state: &AppState, id: Value, _params: Value) -> Value {
    ok(
        id,
        json!({
            "structuredContent": {
                "workspaces": state.config.allowed_workspaces.iter().map(|workspace| {
                    json!({
                        "id": workspace.id,
                        "path": workspace.path,
                        "allowWrite": workspace.allow_write
                    })
                }).collect::<Vec<_>>()
            },
            "content": [{"type": "text", "text": "Configured workspaces returned."}]
        }),
    )
}

pub(crate) fn list_worktrees_tool(state: &AppState, id: Value, params: Value) -> Value {
    let workspace = argument_str(&params, "workspace");
    match list_worktrees(&state.config, workspace) {
        Ok(worktrees) => ok(
            id,
            json!({
                "structuredContent": worktrees,
                "content": [{"type": "text", "text": "Git worktrees returned."}]
            }),
        ),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn create_worktree_tool(state: &AppState, id: Value, params: Value) -> Value {
    let workspace = argument_str(&params, "workspace");
    let name = argument_str(&params, "name");
    let base = argument_str(&params, "base");
    if let Err(err) = append_audit_event(
        &state.config.state_dir,
        &AuditEvent::new(
            "create_worktree",
            format!("workspace={workspace} name={name} base={base}"),
        ),
    ) {
        return error(id, -32602, &err.to_string());
    }
    match create_worktree(&state.config, workspace, name, base) {
        Ok(worktree) => ok(
            id,
            json!({
                "structuredContent": worktree,
                "content": [{"type": "text", "text": "Git worktree created."}]
            }),
        ),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn remove_worktree_tool(state: &AppState, id: Value, params: Value) -> Value {
    let workspace = argument_str(&params, "workspace");
    let name = argument_str(&params, "name");
    if let Err(err) = append_audit_event(
        &state.config.state_dir,
        &AuditEvent::new(
            "remove_worktree",
            format!("workspace={workspace} name={name}"),
        ),
    ) {
        return error(id, -32602, &err.to_string());
    }
    match remove_worktree(&state.config, workspace, name) {
        Ok(worktree) => ok(
            id,
            json!({
                "structuredContent": worktree,
                "content": [{"type": "text", "text": "Git worktree removed."}]
            }),
        ),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn repo_brief_tool(state: &AppState, id: Value, params: Value) -> Value {
    let workspace = argument_str(&params, "workspace");
    match repo_brief(&state.config, workspace) {
        Ok(brief) => ok(
            id,
            json!({
                "structuredContent": brief,
                "content": [{"type": "text", "text": "Repository brief returned."}]
            }),
        ),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn read_context_tool(state: &AppState, id: Value, params: Value) -> Value {
    let workspace = argument_str(&params, "workspace");
    let path = argument_str(&params, "path");
    match read_context(&state.config, workspace, std::path::Path::new(path)) {
        Ok(file) => ok(
            id,
            json!({
                "structuredContent": file,
                "content": [{"type": "text", "text": "File context returned."}]
            }),
        ),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn search_context_tool(state: &AppState, id: Value, params: Value) -> Value {
    let workspace = argument_str(&params, "workspace");
    let query = argument_str(&params, "query");
    match search_context(&state.config, workspace, query) {
        Ok(results) => ok(
            id,
            json!({
                "structuredContent": results,
                "content": [{"type": "text", "text": "Search context returned."}]
            }),
        ),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn search(state: &AppState, id: Value, params: Value) -> Value {
    let workspace = argument_str(&params, "workspace");
    let query = argument_str(&params, "query");
    match compat::search(&state.config, workspace, query) {
        Ok(payload) => compat_ok(id, payload),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn fetch(state: &AppState, id: Value, params: Value) -> Value {
    let document_id = argument_str(&params, "id");
    let document_id = if document_id.is_empty() {
        argument_str(&params, "url")
    } else {
        document_id
    };
    match compat::fetch(&state.config, document_id) {
        Ok(payload) => compat_ok(id, payload),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

fn compat_ok(id: Value, payload: Value) -> Value {
    ok(
        id,
        json!({
            "structuredContent": payload,
            "content": [{"type": "text", "text": payload.to_string()}]
        }),
    )
}

pub(crate) fn run_readonly_smoke_test(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let workspace = match state.config.workspace(workspace_id) {
        Ok(workspace) => workspace,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    let search = match compat::search(&state.config, workspace_id, "OAuth") {
        Ok(search) => search,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    let fetched = search["results"]
        .as_array()
        .and_then(|results| results.first())
        .and_then(|result| result.get("id"))
        .and_then(Value::as_str)
        .map(|document_id| compat::fetch(&state.config, document_id))
        .transpose();
    let fetched = match fetched {
        Ok(Some(fetched)) => fetched,
        Ok(None) => Value::Null,
        Err(err) => return error(id, -32602, &err.to_string()),
    };

    ok(
        id,
        json!({
            "structuredContent": {
                "workspace": {
                    "id": workspace.id,
                    "path": workspace.path,
                    "allowWrite": workspace.allow_write
                },
                "query": "OAuth",
                "search": search,
                "fetched": fetched
            },
            "content": [{"type": "text", "text": "Read-only smoke test returned OAuth search and first fetched result."}]
        }),
    )
}

pub(crate) fn list_codex_threads(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let workspace = match state.config.workspace(workspace_id) {
        Ok(workspace) => workspace,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    let cwd = workspace.path.display().to_string();
    match state.appserver.call("thread/list", json!({"cwd": cwd})) {
        Ok(mut payload) => {
            policy::filter_threads_to_workspace(&mut payload, workspace);
            appserver_ok(id, payload, "Codex threads returned.")
        }
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn start_codex_thread(state: &AppState, id: Value, params: Value) -> Value {
    start_codex_thread_with_sandbox_mode(state, id, params, None, "start_codex_thread")
}

pub(crate) fn start_readonly_codex_thread(state: &AppState, id: Value, params: Value) -> Value {
    start_codex_thread_with_sandbox_mode(
        state,
        id,
        params,
        Some("read-only"),
        "start_readonly_codex_thread",
    )
}

pub(crate) fn start_codex_thread_with_sandbox_mode(
    state: &AppState,
    id: Value,
    params: Value,
    forced_sandbox: Option<&str>,
    audit_action: &str,
) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let prompt = argument_str(&params, "prompt");
    let requested_sandbox = argument_str(&params, "sandbox");
    let sandbox = forced_sandbox.unwrap_or(requested_sandbox);
    let workspace = match policy::workspace_for_run(state, workspace_id, sandbox) {
        Ok(workspace) => workspace,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    if let Err(err) = append_audit_event(
        &state.config.state_dir,
        &AuditEvent::new(
            audit_action,
            format!(
                "workspace={workspace_id} sandbox={}",
                policy::normalized_sandbox(sandbox)
            ),
        ),
    ) {
        return error(id, -32602, &err.to_string());
    }

    let mut appserver_params = json!({
        "cwd": workspace.path,
        "input": text_input(prompt),
    });
    if let Err(err) = policy::merge_run_options(
        &mut appserver_params,
        policy::load_saved_run_options(&state.config, workspace_id).as_ref(),
        &state.config,
        Some(workspace_id),
    ) {
        return error(id, -32602, &err.to_string());
    }
    if let Err(err) = policy::merge_run_options(
        &mut appserver_params,
        argument_value(&params, "options"),
        &state.config,
        Some(workspace_id),
    ) {
        return error(id, -32602, &err.to_string());
    }
    if forced_sandbox.is_some()
        && appserver_params.get("web_search").and_then(Value::as_bool) == Some(true)
    {
        return error(
            id,
            -32602,
            "web_search is not supported for start_readonly_codex_thread",
        );
    }
    if !sandbox.is_empty() || forced_sandbox.is_some() {
        appserver_params["sandbox"] = json!(policy::normalized_sandbox(sandbox));
    }

    let store = RunStore::new(state.config.state_dir.join("runs"));
    let status = match store.create_run(workspace_id, prompt, RunState::Running) {
        Ok(status) => status,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    match state.appserver.call("thread/start", appserver_params) {
        Ok(payload) => {
            let linked = extract_thread_id(&payload)
                .and_then(|thread_id| store.link_thread(&status.run_id, thread_id).ok())
                .unwrap_or(status);
            appserver_ok(
                id,
                json!({"thread": payload.get("thread").cloned().unwrap_or(payload), "run": linked}),
                "Codex thread started.",
            )
        }
        Err(err) => {
            let _ = store.fail_run(&status.run_id, err.to_string());
            error(id, -32602, &err.to_string())
        }
    }
}

pub(crate) fn resume_codex_thread(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    let workspace = match state.config.workspace(workspace_id) {
        Ok(workspace) => workspace,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    if let Err(err) =
        policy::ensure_thread_known_or_visible(state, workspace, workspace_id, thread_id)
    {
        return error(id, -32602, &err.to_string());
    }
    let cwd = match policy::target_cwd(
        &state.config,
        workspace_id,
        argument_str(&params, "worktree"),
    ) {
        Ok(cwd) => cwd,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    match state
        .appserver
        .call("thread/resume", json!({"threadId": thread_id, "cwd": cwd}))
    {
        Ok(payload) => {
            if !policy::payload_cwd_matches_workspace(&payload, workspace) {
                return error(id, -32602, "resumed thread is outside requested workspace");
            }
            let store = RunStore::new(state.config.state_dir.join("runs"));
            if let Ok(status) =
                store.create_run(workspace_id, "resume app-server thread", RunState::Running)
            {
                let _ = store.link_thread(&status.run_id, thread_id);
            }
            appserver_ok(id, payload, "Codex thread resumed.")
        }
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn fork_codex_thread(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    if let Err(err) = state.config.workspace(workspace_id) {
        return error(id, -32602, &err.to_string());
    }
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    let cwd = match policy::target_cwd(
        &state.config,
        workspace_id,
        argument_str(&params, "worktree"),
    ) {
        Ok(cwd) => cwd,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    match state
        .appserver
        .call("thread/fork", json!({"threadId": thread_id, "cwd": cwd}))
    {
        Ok(payload) => {
            if let Some(forked_thread_id) = extract_thread_id(&payload) {
                let store = RunStore::new(state.config.state_dir.join("runs"));
                if let Ok(status) =
                    store.create_run(workspace_id, "fork app-server thread", RunState::Running)
                {
                    let _ = store.link_thread(&status.run_id, forked_thread_id);
                }
            }
            appserver_ok(id, payload, "Codex thread forked.")
        }
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn read_codex_thread(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    appserver_call_tool(
        state,
        id,
        "thread/read",
        json!({"threadId": thread_id}),
        "Codex thread returned.",
    )
}

pub(crate) fn send_codex_turn(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    let prompt = argument_str(&params, "prompt");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    let mut appserver_params = json!({
        "threadId": thread_id,
        "input": text_input(prompt),
    });
    if let Err(err) = policy::merge_run_options(
        &mut appserver_params,
        policy::load_saved_run_options(&state.config, workspace_id).as_ref(),
        &state.config,
        Some(workspace_id),
    ) {
        return error(id, -32602, &err.to_string());
    }
    if let Err(err) = policy::merge_run_options(
        &mut appserver_params,
        argument_value(&params, "options"),
        &state.config,
        Some(workspace_id),
    ) {
        return error(id, -32602, &err.to_string());
    }
    appserver_call_tool(
        state,
        id,
        "turn/start",
        appserver_params,
        "Codex turn started.",
    )
}

pub(crate) fn steer_codex_turn(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    let turn_id = argument_str(&params, "turn_id");
    let prompt = argument_str(&params, "prompt");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    appserver_call_tool(
        state,
        id,
        "turn/steer",
        json!({"threadId": thread_id, "turnId": turn_id, "input": text_input(prompt)}),
        "Codex turn steered.",
    )
}

pub(crate) fn interrupt_codex_turn(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    let turn_id = argument_str(&params, "turn_id");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    appserver_call_tool(
        state,
        id,
        "turn/interrupt",
        json!({"threadId": thread_id, "turnId": turn_id}),
        "Codex turn interrupted.",
    )
}

pub(crate) fn stream_codex_events(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    match state.appserver.events_for_thread(thread_id) {
        Ok(events) => {
            let summary = result_bundle_summary(thread_id, &Value::Null, &events);
            appserver_ok(
                id,
                json!({"thread_id": thread_id, "events": events, "summary": summary}),
                "Codex events returned.",
            )
        }
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn review_codex_thread(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    let workspace = match state.config.workspace(workspace_id) {
        Ok(workspace) => workspace,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    appserver_call_tool(
        state,
        id,
        "review/start",
        json!({"threadId": thread_id, "cwd": workspace.path}),
        "Codex review started.",
    )
}

pub(crate) fn list_models(state: &AppState, id: Value, _params: Value) -> Value {
    match state.appserver.call("model/list", json!({})) {
        Ok(mut payload) => {
            if payload.get("models").is_none() {
                if let Some(data) = payload.get("data").cloned() {
                    payload["models"] = data;
                }
            }
            appserver_ok(id, payload, "Codex models returned.")
        }
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn set_run_options(state: &AppState, id: Value, params: Value) -> Value {
    let Some(options) = argument_value(&params, "options").cloned() else {
        return error(id, -32602, "options is required");
    };
    let Some(object) = options.as_object() else {
        return error(id, -32602, "options must be an object");
    };
    let allowed = [
        "model",
        "reasoning_effort",
        "web_search",
        "extra_read_dirs",
        "images",
        "output_schema",
    ];
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return error(id, -32602, &format!("unsupported run option: {key}"));
    }
    let workspace_id = argument_str(&params, "workspace");
    if workspace_id.is_empty() {
        return error(id, -32602, "workspace is required");
    }
    if let Err(err) = state.config.workspace(workspace_id) {
        return error(id, -32602, &err.to_string());
    }
    if let Err(err) = policy::validate_extra_read_dirs(
        &state.config,
        Some(workspace_id),
        object.get("extra_read_dirs"),
    ) {
        return error(id, -32602, &err.to_string());
    }
    if let Err(err) = policy::validate_workspace_paths(
        &state.config,
        Some(workspace_id),
        "images",
        object.get("images"),
    ) {
        return error(id, -32602, &err.to_string());
    }
    let path = policy::saved_run_options_path(&state.config, workspace_id);
    if let Some(parent) = path.parent()
        && let Err(source) = fs::create_dir_all(parent)
    {
        return error(id, -32602, &source.to_string());
    }
    match serde_json::to_vec_pretty(&options)
        .map_err(|source| source.to_string())
        .and_then(|data| fs::write(&path, data).map_err(|source| source.to_string()))
    {
        Ok(()) => appserver_ok(id, json!({"options": options}), "Run options saved."),
        Err(err) => error(id, -32602, &err),
    }
}

pub(crate) fn run_in_worktree(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let name = argument_str(&params, "name");
    let base = argument_str(&params, "base");
    let prompt = argument_str(&params, "prompt");
    if let Err(err) = append_audit_event(
        &state.config.state_dir,
        &AuditEvent::new(
            "run_in_worktree",
            format!("workspace={workspace_id} name={name} base={base}"),
        ),
    ) {
        return error(id, -32602, &err.to_string());
    }
    let created = match create_worktree(&state.config, workspace_id, name, base) {
        Ok(created) => created,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    let workspace = match state.config.workspace(workspace_id) {
        Ok(workspace) => workspace,
        Err(err) => {
            cleanup_worktree(&state.config, workspace_id, name);
            return error(id, -32602, &err.to_string());
        }
    };
    let root = match workspace.path.canonicalize() {
        Ok(root) => root,
        Err(err) => {
            cleanup_worktree(&state.config, workspace_id, name);
            return error(id, -32602, &err.to_string());
        }
    };
    let parent = match root.parent() {
        Some(parent) => parent,
        None => {
            cleanup_worktree(&state.config, workspace_id, name);
            return error(id, -32602, "workspace has no parent");
        }
    };
    let cwd = parent.join(&created.path);
    let mut appserver_params =
        json!({"cwd": cwd, "input": text_input(prompt), "sandbox": "workspace-write"});
    if let Err(err) = policy::merge_run_options(
        &mut appserver_params,
        policy::load_saved_run_options(&state.config, workspace_id).as_ref(),
        &state.config,
        Some(workspace_id),
    ) {
        cleanup_worktree(&state.config, workspace_id, name);
        return error(id, -32602, &err.to_string());
    }
    let store = RunStore::new(state.config.state_dir.join("runs"));
    let status = match store.create_run(workspace_id, prompt, RunState::Running) {
        Ok(status) => status,
        Err(err) => {
            cleanup_worktree(&state.config, workspace_id, name);
            return error(id, -32602, &err.to_string());
        }
    };
    match state.appserver.call("thread/start", appserver_params) {
        Ok(thread) => {
            let linked = extract_thread_id(&thread)
                .and_then(|thread_id| store.link_thread(&status.run_id, thread_id).ok())
                .unwrap_or(status);
            appserver_ok(
                id,
                json!({"worktree": created, "thread": thread, "run": linked}),
                "Worktree thread started.",
            )
        }
        Err(err) => {
            let _ = store.fail_run(&status.run_id, err.to_string());
            cleanup_worktree(&state.config, workspace_id, name);
            error(id, -32602, &err.to_string())
        }
    }
}

fn cleanup_worktree(config: &AppConfig, workspace_id: &str, name: &str) {
    let _ = remove_worktree(config, workspace_id, name);
}

pub(crate) fn list_hooks_skills_mcp(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let workspace = match state.config.workspace(workspace_id) {
        Ok(workspace) => workspace,
        Err(err) => return error(id, -32602, &err.to_string()),
    };
    let cwd = workspace.path.display().to_string();
    let config = optional_appserver_call(state, "config/get", json!({"cwd": cwd}));
    let config = config.get("config").cloned().unwrap_or(config);
    let mcp = optional_appserver_call(state, "mcp/list", json!({"cwd": cwd}));
    let features = optional_appserver_call(state, "features/list", json!({"cwd": cwd}));
    let skills = optional_appserver_call(state, "skills/list", json!({"cwd": cwd}));
    let plugins = optional_appserver_call(state, "plugins/list", json!({"cwd": cwd}));
    let hooks = optional_appserver_call(state, "hooks/list", json!({"cwd": cwd}));
    appserver_ok(
        id,
        json!({
            "config": config,
            "mcp": mcp,
            "features": features,
            "skills": skills,
            "plugins": plugins,
            "hooks": hooks,
        }),
        "Codex capabilities returned.",
    )
}

pub(crate) fn approval_bridge(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    if workspace_id.is_empty() {
        return error(id, -32602, "workspace is required");
    }
    if let Err(err) = state.config.workspace(workspace_id) {
        return error(id, -32602, &err.to_string());
    }
    if let Some(request_id) = argument_value(&params, "request_id").cloned() {
        let pending = match state.appserver.pending_requests() {
            Ok(pending) => pending,
            Err(err) => return error(id, -32602, &err.to_string()),
        };
        let request_allowed = pending.iter().any(|request| {
            request.get("id").is_some_and(|id| *id == request_id)
                && policy::approval_request_allowed_for_workspace(state, workspace_id, request)
        });
        if !request_allowed {
            return error(
                id,
                -32602,
                "approval request is not visible in requested workspace",
            );
        }
        let decision = argument_str(&params, "decision");
        if decision != "deny" {
            return error(
                id,
                -32602,
                "approval_bridge can only deny pending approvals",
            );
        }
        let reason = argument_str(&params, "reason");
        if let Err(err) = state.appserver.respond_value(
            request_id.clone(),
            json!({"decision": decision, "reason": reason}),
        ) {
            return error(id, -32602, &err.to_string());
        }
        return appserver_ok(
            id,
            json!({"request_id": request_id, "decision": decision}),
            "Approval response sent.",
        );
    }

    match state.appserver.pending_requests() {
        Ok(pending) => {
            let pending = pending
                .into_iter()
                .filter(|request| {
                    policy::approval_request_allowed_for_workspace(state, workspace_id, request)
                })
                .collect::<Vec<_>>();
            appserver_ok(
                id,
                json!({"pending": pending}),
                "Pending approvals returned.",
            )
        }
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn export_result_bundle(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    let thread = optional_appserver_call(state, "thread/read", json!({"threadId": thread_id}));
    match state.appserver.events_for_thread(thread_id) {
        Ok(events) => {
            let summary = result_bundle_summary(thread_id, &thread, &events);
            appserver_ok(id, summary, "Result bundle exported.")
        }
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

pub(crate) fn thread_lifecycle_call(
    state: &AppState,
    id: Value,
    params: Value,
    method: &str,
    message: &str,
) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    appserver_call_tool(state, id, method, json!({"threadId": thread_id}), message)
}

pub(crate) fn terminate_background_terminal(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    let Some(process_id) = argument_value(&params, "process_id").and_then(Value::as_i64) else {
        return error(id, -32602, "process_id is required");
    };
    appserver_call_tool(
        state,
        id,
        "thread/backgroundTerminals/terminate",
        json!({"threadId": thread_id, "processId": process_id}),
        "Background terminal terminated.",
    )
}

pub(crate) fn rollback_thread(state: &AppState, id: Value, params: Value) -> Value {
    let workspace_id = argument_str(&params, "workspace");
    let thread_id = argument_str(&params, "thread_id");
    if let Err(err) = policy::ensure_thread_allowed(state, workspace_id, thread_id) {
        return error(id, -32602, &err.to_string());
    }
    let turns = argument_value(&params, "turns")
        .and_then(Value::as_u64)
        .unwrap_or(1);
    appserver_call_tool(
        state,
        id,
        "thread/rollback",
        json!({"threadId": thread_id, "turns": turns}),
        "Thread rolled back.",
    )
}

fn extract_thread_id(payload: &Value) -> Option<&str> {
    payload
        .get("thread")
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("threadId").and_then(Value::as_str))
        .or_else(|| payload.get("thread_id").and_then(Value::as_str))
}

fn result_bundle_summary(thread_id: &str, thread: &Value, events: &[Value]) -> Value {
    let mut final_message = Value::Null;
    let mut status = Value::String("unknown".to_owned());
    let mut token_usage = Value::Null;
    let mut branch = Value::Null;
    let mut commands = Vec::new();
    let mut tests = Vec::new();
    let mut changed_files = Vec::new();
    let mut diff_summary = Vec::new();

    for event in events {
        let payload = event.get("payload").unwrap_or(event);
        let params = payload.get("params").unwrap_or(payload);
        if branch.is_null() {
            if let Some(branch_name) =
                first_string(params, &["branch", "currentBranch", "current_branch"])
            {
                branch = Value::String(branch_name.to_owned());
            }
        }
        if let Some(event_status) = params.get("status").and_then(Value::as_str) {
            status = Value::String(event_status.to_owned());
        }
        if let Some(turn_status) = params
            .get("turn")
            .and_then(|turn| turn.get("status"))
            .and_then(Value::as_str)
        {
            status = Value::String(turn_status.to_owned());
        }
        if let Some(usage) = params.get("usage") {
            token_usage = usage.clone();
        }
        let item = params.get("item").unwrap_or(params);
        if branch.is_null() {
            if let Some(branch_name) =
                first_string(item, &["branch", "currentBranch", "current_branch"])
            {
                branch = Value::String(branch_name.to_owned());
            }
        }
        if item.get("type").and_then(Value::as_str) == Some("agent_message") {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                final_message = Value::String(text.to_owned());
            }
        } else if item.get("phase").and_then(Value::as_str) == Some("final_answer")
            && let Some(text) = item.get("text").and_then(Value::as_str)
        {
            final_message = Value::String(text.to_owned());
        }
        if let Some(command) = item
            .get("command")
            .or_else(|| params.get("command"))
            .and_then(Value::as_str)
        {
            push_unique(&mut commands, command);
            if command.contains("test") {
                push_unique(&mut tests, command);
            }
        }
        if let Some(path) = item
            .get("path")
            .or_else(|| params.get("path"))
            .and_then(Value::as_str)
        {
            push_unique(&mut changed_files, path);
        }
        if let Some(files) = item
            .get("files")
            .or_else(|| params.get("files"))
            .and_then(Value::as_array)
        {
            for file in files.iter().filter_map(Value::as_str) {
                push_unique(&mut changed_files, file);
            }
        }
        if let Some(summary) = first_string(item, &["diffSummary", "diff_summary", "summary"]) {
            push_unique(&mut diff_summary, summary);
        } else if let Some(summary) =
            first_string(params, &["diffSummary", "diff_summary", "summary"])
        {
            push_unique(&mut diff_summary, summary);
        }
    }

    json!({
        "thread_id": thread_id,
        "thread": thread,
        "events": events,
        "final_message": final_message,
        "changed_files": changed_files,
        "branch": branch,
        "diff_summary": diff_summary,
        "commands_run": commands,
        "tests_run": tests,
        "status": status,
        "token_usage": token_usage,
    })
}

fn first_string<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_owned());
    }
}

fn argument_str<'a>(params: &'a Value, key: &str) -> &'a str {
    params
        .get("arguments")
        .and_then(|arguments| arguments.get(key))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn argument_value<'a>(params: &'a Value, key: &str) -> Option<&'a Value> {
    params
        .get("arguments")
        .and_then(|arguments| arguments.get(key))
}

fn text_input(text: &str) -> Value {
    json!([{"type": "text", "text": text}])
}

fn appserver_ok(id: Value, payload: Value, text: &str) -> Value {
    ok(
        id,
        json!({
            "structuredContent": payload,
            "content": [{"type": "text", "text": text}]
        }),
    )
}

fn appserver_call_tool(
    state: &AppState,
    id: Value,
    method: &str,
    params: Value,
    text: &str,
) -> Value {
    match state.appserver.call(method, params) {
        Ok(payload) => appserver_ok(id, payload, text),
        Err(err) => error(id, -32602, &err.to_string()),
    }
}

fn optional_appserver_call(state: &AppState, method: &str, params: Value) -> Value {
    match state.appserver.call(method, params) {
        Ok(payload) => payload,
        Err(err) => json!({"error": err.to_string()}),
    }
}
