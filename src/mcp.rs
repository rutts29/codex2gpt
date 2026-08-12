use serde_json::{Value, json};

use crate::APP_NAME;
use crate::appserver::AppServerClient;
use crate::config::AppConfig;

mod compat;
mod handlers;
mod input_schemas;
mod output_schemas;
mod policy;
mod registry;
mod schema;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: AppConfig,
    appserver: AppServerClient,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let appserver = AppServerClient::new(
            config.codex_binary.clone(),
            config.state_dir.join("appserver"),
        );
        Self { config, appserver }
    }
}

pub fn handle_json_rpc(state: &AppState, request: Value) -> Value {
    let Some(object) = request.as_object() else {
        return error(Value::Null, -32600, "Invalid Request");
    };
    let is_notification = !object.contains_key("id");
    let id = object.get("id").cloned().unwrap_or(Value::Null);
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return error(id, -32600, "Invalid Request");
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return error(id, -32600, "Invalid Request");
    };
    if is_notification {
        return Value::Null;
    }

    match method {
        "initialize" => ok(
            id,
            json!({
                "protocolVersion": "2025-06-18",
                "serverInfo": {"name": APP_NAME, "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"tools": {}, "resources": {}}
            }),
        ),
        "tools/list" => ok(id, json!({"tools": registry::descriptors(&state.config)})),
        "resources/list" => ok(
            id,
            json!({"resources": crate::widgets::resource_descriptors(&state.config)}),
        ),
        "resources/read" => read_widget_resource(
            state,
            id,
            object.get("params").cloned().unwrap_or(Value::Null),
        ),
        "tools/call" => call_tool(
            state,
            id,
            object.get("params").cloned().unwrap_or(Value::Null),
        ),
        _ => error(id, -32601, "Method not found"),
    }
}

fn read_widget_resource(state: &AppState, id: Value, params: Value) -> Value {
    let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
    match crate::widgets::read_resource(&state.config, uri) {
        Ok(resource) => ok(id, resource),
        Err(message) => error(id, -32602, message),
    }
}

fn call_tool(state: &AppState, id: Value, params: Value) -> Value {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    registry::call(state, id, &name, params)
}

pub(crate) fn ok(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

pub(crate) fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
