use serde_json::{Value, json};

use super::schema::*;

pub(crate) fn list_workspaces_output_schema() -> Value {
    object_schema(
        json!({
            "workspaces": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "path": {"type": "string"},
                        "allowWrite": {"type": "boolean"}
                    },
                    "required": ["id", "path", "allowWrite"],
                    "additionalProperties": false
                }
            }
        }),
        &["workspaces"],
        false,
    )
}

pub(crate) fn list_worktrees_output_schema() -> Value {
    object_schema(
        json!({
            "workspace_id": {"type": "string"},
            "worktrees": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "branch": {"type": ["string", "null"]},
                        "commit": {"type": ["string", "null"]}
                    },
                    "required": ["path", "branch", "commit"],
                    "additionalProperties": false
                }
            }
        }),
        &["workspace_id", "worktrees"],
        false,
    )
}

pub(crate) fn create_worktree_output_schema() -> Value {
    object_schema(
        json!({
            "workspace_id": {"type": "string"},
            "name": {"type": "string"},
            "branch": {"type": "string"},
            "base": {"type": "string"},
            "path": {"type": "string"}
        }),
        &["workspace_id", "name", "branch", "base", "path"],
        false,
    )
}

pub(crate) fn remove_worktree_output_schema() -> Value {
    object_schema(
        json!({"workspace_id": {"type": "string"}, "name": {"type": "string"}, "path": {"type": "string"}}),
        &["workspace_id", "name", "path"],
        false,
    )
}

pub(crate) fn repo_brief_output_schema() -> Value {
    object_schema(
        json!({
            "workspace_id": {"type": "string"},
            "root": {"type": "string"},
            "has_git_dir": {"type": "boolean"},
            "entries": {"type": "array", "items": {"type": "string"}}
        }),
        &["workspace_id", "root", "has_git_dir", "entries"],
        false,
    )
}

pub(crate) fn read_context_output_schema() -> Value {
    object_schema(
        json!({"path": {"type": "string"}, "text": {"type": "string"}, "truncated": {"type": "boolean"}}),
        &["path", "text", "truncated"],
        false,
    )
}

pub(crate) fn search_context_output_schema() -> Value {
    object_schema(
        json!({
            "query": {"type": "string"},
            "matches": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "line": {"type": "integer"},
                        "text": {"type": "string"}
                    },
                    "required": ["path", "line", "text"],
                    "additionalProperties": false
                }
            },
            "truncated": {"type": "boolean"}
        }),
        &["query", "matches", "truncated"],
        false,
    )
}

pub(crate) fn search_output_schema() -> Value {
    object_schema(
        json!({
            "results": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "title": {"type": "string"},
                        "url": {"type": "string"}
                    },
                    "required": ["id", "title", "url"],
                    "additionalProperties": false
                }
            }
        }),
        &["results"],
        false,
    )
}

pub(crate) fn fetch_output_schema() -> Value {
    object_schema(
        json!({
            "id": {"type": "string"},
            "title": {"type": "string"},
            "text": {"type": "string"},
            "url": {"type": "string"},
            "metadata": {
                "type": "object",
                "properties": {
                    "workspace": {"type": "string"},
                    "path": {"type": "string"},
                    "truncated": {"type": "boolean"}
                },
                "required": ["workspace", "path", "truncated"],
                "additionalProperties": false
            }
        }),
        &["id", "title", "text", "url", "metadata"],
        false,
    )
}

pub(crate) fn readonly_smoke_output_schema() -> Value {
    object_schema(
        json!({
            "workspace": {
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "path": {"type": "string"},
                    "allowWrite": {"type": "boolean"}
                },
                "required": ["id", "path", "allowWrite"],
                "additionalProperties": false
            },
            "query": {"type": "string"},
            "search": appserver_object_schema(),
            "fetched": appserver_value_schema()
        }),
        &["workspace", "query", "search", "fetched"],
        false,
    )
}

pub(crate) fn list_codex_threads_output_schema() -> Value {
    object_schema(
        json!({"threads": {"type": "array", "items": appserver_object_schema()}}),
        &["threads"],
        true,
    )
}

pub(crate) fn start_thread_output_schema() -> Value {
    object_schema(
        json!({"thread": appserver_object_schema(), "run": run_status_schema(), "worktree": appserver_object_schema()}),
        &[],
        true,
    )
}

pub(crate) fn thread_output_schema() -> Value {
    object_schema(json!({"thread": appserver_object_schema()}), &[], true)
}

pub(crate) fn send_turn_output_schema() -> Value {
    object_schema(
        json!({"threadId": {"type": "string"}, "turnId": {"type": "string"}, "turn": appserver_object_schema()}),
        &[],
        true,
    )
}

pub(crate) fn steer_turn_output_schema() -> Value {
    object_schema(json!({"turnId": {"type": "string"}}), &[], true)
}

pub(crate) fn interrupt_turn_output_schema() -> Value {
    object_schema(json!({"interrupted": {"type": "boolean"}}), &[], true)
}

pub(crate) fn stream_events_output_schema() -> Value {
    object_schema(
        json!({"thread_id": {"type": "string"}, "events": {"type": "array", "items": appserver_object_schema()}}),
        &["thread_id", "events"],
        false,
    )
}

pub(crate) fn review_thread_output_schema() -> Value {
    object_schema(
        json!({"review": appserver_object_schema(), "thread": appserver_object_schema()}),
        &[],
        true,
    )
}

pub(crate) fn list_models_output_schema() -> Value {
    object_schema(
        json!({"models": {"type": "array", "items": appserver_object_schema()}}),
        &["models"],
        true,
    )
}

pub(crate) fn set_run_options_output_schema() -> Value {
    object_schema(
        json!({"options": run_options_schema()}),
        &["options"],
        false,
    )
}

pub(crate) fn list_hooks_skills_mcp_output_schema() -> Value {
    object_schema(
        json!({
            "config": appserver_value_schema(),
            "mcp": appserver_value_schema(),
            "skills": appserver_value_schema(),
            "plugins": appserver_value_schema(),
            "hooks": appserver_value_schema()
        }),
        &["config", "mcp", "skills", "plugins", "hooks"],
        false,
    )
}

pub(crate) fn approval_output_schema() -> Value {
    object_schema(
        json!({
            "pending": {"type": "array", "items": appserver_object_schema()},
            "request_id": appserver_value_schema(),
            "decision": {"type": "string", "enum": ["deny"]}
        }),
        &[],
        false,
    )
}

pub(crate) fn export_result_bundle_output_schema() -> Value {
    object_schema(
        json!({
            "thread_id": {"type": "string"},
            "thread": appserver_value_schema(),
            "events": {"type": "array", "items": appserver_object_schema()},
            "final_message": appserver_value_schema(),
            "changed_files": {"type": "array", "items": {"type": "string"}},
            "branch": appserver_value_schema(),
            "diff_summary": {"type": "array", "items": {"type": "string"}},
            "commands_run": {"type": "array", "items": {"type": "string"}},
            "tests_run": {"type": "array", "items": {"type": "string"}},
            "status": {"type": "string"},
            "token_usage": appserver_value_schema()
        }),
        &[
            "thread_id",
            "thread",
            "events",
            "final_message",
            "changed_files",
            "branch",
            "diff_summary",
            "commands_run",
            "tests_run",
            "status",
            "token_usage",
        ],
        false,
    )
}

pub(crate) fn list_background_terminals_output_schema() -> Value {
    object_schema(
        json!({"terminals": {"type": "array", "items": appserver_object_schema()}}),
        &[],
        true,
    )
}

pub(crate) fn terminate_background_terminal_output_schema() -> Value {
    object_schema(
        json!({"processId": {"type": "integer"}, "terminated": {"type": "boolean"}}),
        &[],
        true,
    )
}

pub(crate) fn clean_background_terminals_output_schema() -> Value {
    object_schema(json!({"cleaned": {"type": "boolean"}}), &[], true)
}

pub(crate) fn rollback_thread_output_schema() -> Value {
    object_schema(json!({"thread": appserver_object_schema()}), &[], true)
}

pub(crate) fn lifecycle_output_schema() -> Value {
    object_schema(
        json!({"threadId": {"type": "string"}, "thread_id": {"type": "string"}, "status": {"type": "string"}}),
        &[],
        true,
    )
}
