use serde_json::{Value, json};

pub(crate) fn run_status_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "run_id": {"type": "string"},
            "workspace_id": {"type": "string"},
            "state": {"type": "string", "enum": ["running", "completed", "failed", "canceled"]},
            "final_message": {"type": ["string", "null"]},
            "thread_id": {"type": ["string", "null"]}
        },
        "required": ["run_id", "workspace_id", "state", "final_message", "thread_id"],
        "additionalProperties": false
    })
}

pub(crate) fn object_schema(
    properties: Value,
    required: &[&str],
    additional_properties: bool,
) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": additional_properties
    })
}

pub(crate) fn appserver_object_schema() -> Value {
    json!({"type": "object", "additionalProperties": true})
}

pub(crate) fn appserver_value_schema() -> Value {
    json!({"type": ["object", "array", "string", "number", "integer", "boolean", "null"]})
}

pub(crate) fn workspace_thread_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "thread_id": {"type": "string"}
    })
}

pub(crate) fn workspace_thread_target_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "thread_id": {"type": "string"},
        "worktree": {"type": "string", "pattern": "^[A-Za-z0-9_-]{1,80}$"}
    })
}

pub(crate) fn thread_start_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "prompt": {"type": "string"},
        "sandbox": {"type": "string", "enum": ["read-only", "workspace-write"]},
        "options": run_options_schema()
    })
}

pub(crate) fn turn_start_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "thread_id": {"type": "string"},
        "prompt": {"type": "string"},
        "options": run_options_schema()
    })
}

pub(crate) fn turn_steer_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "thread_id": {"type": "string"},
        "turn_id": {"type": "string"},
        "prompt": {"type": "string"}
    })
}

pub(crate) fn turn_interrupt_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "thread_id": {"type": "string"},
        "turn_id": {"type": "string"}
    })
}

pub(crate) fn terminate_terminal_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "thread_id": {"type": "string"},
        "process_id": {"type": "integer"}
    })
}

pub(crate) fn rollback_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "thread_id": {"type": "string"},
        "turns": {"type": "integer", "minimum": 1}
    })
}

pub(crate) fn run_in_worktree_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "name": {"type": "string", "pattern": "^[A-Za-z0-9_-]{1,80}$"},
        "base": {"type": "string"},
        "prompt": {"type": "string"}
    })
}

pub(crate) fn approval_properties() -> Value {
    json!({
        "workspace": {"type": "string"},
        "request_id": {"type": ["integer", "string"]},
        "decision": {"type": "string", "enum": ["deny"]},
        "reason": {"type": "string"}
    })
}

pub(crate) fn run_options_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "model": {"type": "string"},
            "reasoning_effort": {"type": "string"},
            "web_search": {"type": "boolean"},
            "extra_read_dirs": {"type": "array", "items": {"type": "string"}},
            "images": {"type": "array", "items": {"type": "string"}},
            "output_schema": {"type": "object"}
        },
        "additionalProperties": false
    })
}
