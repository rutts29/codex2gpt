use serde_json::{Value, json};

use super::schema::*;

pub(crate) fn empty_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    })
}

pub(crate) fn codex_empty_input_schema() -> Value {
    object_schema(json!({}), &[], false)
}

pub(crate) fn workspace_input_schema() -> Value {
    object_schema(
        json!({"workspace": {"type": "string"}}),
        &["workspace"],
        false,
    )
}

pub(crate) fn search_input_schema() -> Value {
    object_schema(
        json!({"workspace": {"type": "string"}, "query": {"type": "string"}}),
        &["workspace", "query"],
        false,
    )
}

pub(crate) fn fetch_input_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": {"type": "string"},
            "url": {"type": "string"}
        },
        "anyOf": [
            {"required": ["id"]},
            {"required": ["url"]}
        ],
        "additionalProperties": false
    })
}

pub(crate) fn create_worktree_input_schema() -> Value {
    object_schema(
        json!({
            "workspace": {"type": "string"},
            "name": {"type": "string", "pattern": "^[A-Za-z0-9_-]{1,80}$"},
            "base": {"type": "string"}
        }),
        &["workspace", "name", "base"],
        false,
    )
}

pub(crate) fn remove_worktree_input_schema() -> Value {
    object_schema(
        json!({
            "workspace": {"type": "string"},
            "name": {"type": "string", "pattern": "^[A-Za-z0-9_-]{1,80}$"}
        }),
        &["workspace", "name"],
        false,
    )
}

pub(crate) fn read_context_input_schema() -> Value {
    object_schema(
        json!({"workspace": {"type": "string"}, "path": {"type": "string"}}),
        &["workspace", "path"],
        false,
    )
}

pub(crate) fn set_run_options_input_schema() -> Value {
    object_schema(
        json!({"workspace": {"type": "string"}, "options": run_options_schema()}),
        &["options"],
        false,
    )
}

pub(crate) fn workspace_thread_input_schema() -> Value {
    object_schema(
        workspace_thread_properties(),
        &["workspace", "thread_id"],
        false,
    )
}

pub(crate) fn workspace_thread_target_input_schema() -> Value {
    object_schema(
        workspace_thread_target_properties(),
        &["workspace", "thread_id"],
        false,
    )
}

pub(crate) fn thread_start_input_schema() -> Value {
    object_schema(thread_start_properties(), &["workspace", "prompt"], false)
}

pub(crate) fn turn_start_input_schema() -> Value {
    object_schema(
        turn_start_properties(),
        &["workspace", "thread_id", "prompt"],
        false,
    )
}

pub(crate) fn turn_steer_input_schema() -> Value {
    object_schema(
        turn_steer_properties(),
        &["workspace", "thread_id", "turn_id", "prompt"],
        false,
    )
}

pub(crate) fn turn_interrupt_input_schema() -> Value {
    object_schema(
        turn_interrupt_properties(),
        &["workspace", "thread_id", "turn_id"],
        false,
    )
}

pub(crate) fn terminate_terminal_input_schema() -> Value {
    object_schema(
        terminate_terminal_properties(),
        &["workspace", "thread_id", "process_id"],
        false,
    )
}

pub(crate) fn rollback_input_schema() -> Value {
    object_schema(rollback_properties(), &["workspace", "thread_id"], false)
}

pub(crate) fn run_in_worktree_input_schema() -> Value {
    object_schema(
        run_in_worktree_properties(),
        &["workspace", "name", "base", "prompt"],
        false,
    )
}

pub(crate) fn approval_input_schema() -> Value {
    object_schema(approval_properties(), &["workspace"], false)
}
