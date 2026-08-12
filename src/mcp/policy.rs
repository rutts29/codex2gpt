use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use crate::config::{AppConfig, WorkspaceConfig};
use crate::runs::RunStore;
use crate::worktrees::managed_worktree_path;

use super::AppState;

pub(crate) fn target_cwd(
    config: &AppConfig,
    workspace_id: &str,
    worktree_name: &str,
) -> crate::error::Result<std::path::PathBuf> {
    if worktree_name.is_empty() {
        return Ok(config.workspace(workspace_id)?.path.clone());
    }
    managed_worktree_path(config, workspace_id, worktree_name)
}

pub(crate) fn workspace_for_run<'a>(
    state: &'a AppState,
    workspace_id: &str,
    sandbox: &str,
) -> crate::error::Result<&'a WorkspaceConfig> {
    let workspace = state.config.workspace(workspace_id)?;
    if normalized_sandbox(sandbox) == "workspace-write" && !workspace.allow_write {
        return Err(crate::error::AppError::WorkspaceReadOnly(
            workspace.id.clone(),
        ));
    }
    Ok(workspace)
}

pub(crate) fn normalized_sandbox(sandbox: &str) -> &'static str {
    match sandbox {
        "workspace-write" => "workspace-write",
        _ => "read-only",
    }
}

pub(crate) fn merge_run_options(
    target: &mut Value,
    options: Option<&Value>,
    config: &AppConfig,
    workspace_id: Option<&str>,
) -> crate::error::Result<()> {
    let Some(options) = options.and_then(Value::as_object) else {
        return Ok(());
    };
    validate_extra_read_dirs(config, workspace_id, options.get("extra_read_dirs"))?;
    validate_workspace_paths(config, workspace_id, "images", options.get("images"))?;
    let Some(target) = target.as_object_mut() else {
        return Ok(());
    };
    for key in [
        "model",
        "reasoning_effort",
        "web_search",
        "extra_read_dirs",
        "images",
        "output_schema",
    ] {
        if let Some(value) = options.get(key) {
            target.insert(key.to_owned(), value.clone());
        }
    }
    Ok(())
}

pub(crate) fn load_saved_run_options(config: &AppConfig, workspace_id: &str) -> Option<Value> {
    let raw = fs::read_to_string(saved_run_options_path(config, workspace_id)).ok()?;
    serde_json::from_str(&raw).ok()
}

pub(crate) fn saved_run_options_path(config: &AppConfig, workspace_id: &str) -> std::path::PathBuf {
    config
        .state_dir
        .join("run-options")
        .join(URL_SAFE_NO_PAD.encode(workspace_id.as_bytes()))
        .with_extension("json")
}

pub(crate) fn filter_threads_to_workspace(payload: &mut Value, workspace: &WorkspaceConfig) {
    let Some(threads) = payload.get_mut("threads").and_then(Value::as_array_mut) else {
        return;
    };
    threads.retain(|thread| match thread.get("cwd").and_then(Value::as_str) {
        Some(cwd) => cwd_allowed_for_workspace(cwd, workspace),
        None => false,
    });
}

pub(crate) fn payload_cwd_matches_workspace(payload: &Value, workspace: &WorkspaceConfig) -> bool {
    let Some(cwd) = payload
        .get("thread")
        .and_then(|thread| thread.get("cwd"))
        .or_else(|| payload.get("cwd"))
        .and_then(Value::as_str)
    else {
        return false;
    };
    cwd_allowed_for_workspace(cwd, workspace)
}

fn cwd_allowed_for_workspace(cwd: &str, workspace: &WorkspaceConfig) -> bool {
    let root = workspace
        .path
        .canonicalize()
        .unwrap_or_else(|_| workspace.path.clone());
    let candidate = Path::new(cwd);
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }
    if candidate == root || candidate == workspace.path {
        return true;
    }
    let Some(parent) = root.parent() else {
        return false;
    };
    let managed_root = parent.join(".codex2gpt-worktrees").join(&workspace.id);
    if let (Ok(canonical_candidate), Ok(canonical_managed_root)) =
        (candidate.canonicalize(), managed_root.canonicalize())
    {
        return canonical_candidate.starts_with(canonical_managed_root);
    }
    candidate.starts_with(managed_root)
}

pub(crate) fn ensure_thread_allowed(
    state: &AppState,
    workspace_id: &str,
    thread_id: &str,
) -> crate::error::Result<()> {
    state.config.workspace(workspace_id)?;
    let store = RunStore::new(state.config.state_dir.join("runs"));
    match store.workspace_for_thread(thread_id)? {
        Some(linked_workspace) if linked_workspace == workspace_id => Ok(()),
        Some(_) => Err(crate::error::AppError::CodexCommand(
            "thread is linked to a different workspace".to_owned(),
        )),
        None => Err(crate::error::AppError::CodexCommand(
            "thread is not linked to the requested workspace".to_owned(),
        )),
    }
}

pub(crate) fn approval_request_allowed_for_workspace(
    state: &AppState,
    workspace_id: &str,
    request: &Value,
) -> bool {
    request
        .get("params")
        .and_then(|params| {
            params
                .get("threadId")
                .or_else(|| params.get("thread_id"))
                .and_then(Value::as_str)
        })
        .is_some_and(|thread_id| ensure_thread_allowed(state, workspace_id, thread_id).is_ok())
}

pub(crate) fn ensure_thread_known_or_visible(
    state: &AppState,
    workspace: &WorkspaceConfig,
    workspace_id: &str,
    thread_id: &str,
) -> crate::error::Result<()> {
    let store = RunStore::new(state.config.state_dir.join("runs"));
    match store.workspace_for_thread(thread_id)? {
        Some(linked_workspace) if linked_workspace == workspace_id => return Ok(()),
        Some(_) => {
            return Err(crate::error::AppError::CodexCommand(
                "thread is linked to a different workspace".to_owned(),
            ));
        }
        None => {}
    }

    let cwd = workspace.path.display().to_string();
    let mut payload = state.appserver.call("thread/list", json!({"cwd": cwd}))?;
    filter_threads_to_workspace(&mut payload, workspace);
    let visible = payload
        .get("threads")
        .and_then(Value::as_array)
        .is_some_and(|threads| {
            threads
                .iter()
                .any(|thread| thread.get("id").and_then(Value::as_str) == Some(thread_id))
        });
    if visible {
        Ok(())
    } else {
        Err(crate::error::AppError::CodexCommand(
            "thread is not linked to or visible in the requested workspace".to_owned(),
        ))
    }
}

pub(crate) fn validate_extra_read_dirs(
    config: &AppConfig,
    workspace_id: Option<&str>,
    value: Option<&Value>,
) -> crate::error::Result<()> {
    validate_workspace_paths(config, workspace_id, "extra_read_dirs", value)
}

pub(crate) fn validate_workspace_paths(
    config: &AppConfig,
    workspace_id: Option<&str>,
    key: &str,
    value: Option<&Value>,
) -> crate::error::Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let Some(workspace_id) = workspace_id else {
        return Err(crate::error::AppError::CodexCommand(format!(
            "{key} requires a workspace"
        )));
    };
    let workspace = config.workspace(workspace_id)?;
    let root =
        workspace
            .path
            .canonicalize()
            .map_err(|source| crate::error::AppError::ReadFile {
                path: workspace.path.clone(),
                source,
            })?;
    let Some(dirs) = value.as_array() else {
        return Err(crate::error::AppError::CodexCommand(format!(
            "{key} must be an array"
        )));
    };
    for dir in dirs {
        let Some(dir) = dir.as_str() else {
            return Err(crate::error::AppError::CodexCommand(format!(
                "{key} entries must be strings"
            )));
        };
        let candidate = if Path::new(dir).is_absolute() {
            Path::new(dir).to_path_buf()
        } else {
            workspace.path.join(dir)
        };
        let canonical =
            candidate
                .canonicalize()
                .map_err(|source| crate::error::AppError::ReadFile {
                    path: candidate.clone(),
                    source,
                })?;
        if !canonical.starts_with(&root) {
            return Err(crate::error::AppError::CodexCommand(format!(
                "{key} path escapes workspace: {}",
                candidate.display()
            )));
        }
    }
    Ok(())
}
