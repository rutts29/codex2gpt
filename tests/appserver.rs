use std::fs;
use std::path::Path;

use codex2gpt::appserver::AppServerClient;
use serde_json::{Value, json};

#[cfg(unix)]
fn write_fake_appserver(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let body = format!(
        r#"#!/usr/bin/env python3
import json
import os
import sys

log = os.path.join(os.path.dirname(sys.argv[0]), "messages.jsonl")


def log_input(data):
    with open(log, "a", encoding="utf-8") as out:
        out.write(json.dumps(data))
        out.write("\n")


def send(obj):
    print(json.dumps(obj), flush=True)


for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    log_input(message)
    method = message.get("method")

    if method == "initialize":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"protocolVersion":"2025-06-18","serverInfo":{{"name":"fake-app-server","version":"0.1.0"}}}}}})
        continue
    if method == "initialized":
        continue
    if method == "thread/start":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"thread":{{"id":"thr_test","status":"active","turns":[]}}}}}})
        send({{"jsonrpc":"2.0","method":"thread/started","params":{{"threadId":"thr_test","status":"active"}}}})
        continue
    if method == "thread/resume":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"thread":{{"id":"thr_test","status":"active","turns":[]}}}}}})
        continue
    if method == "thread/fork":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"thread":{{"id":"thr_fork","status":"active","turns":[]}}}}}})
        continue
    if method == "turn/start":
        thread_id = message.get("params", {{}}).get("threadId", "thr_test")
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"turn":{{"id":"turn_1","status":"completed","threadId":thread_id}}}}}})
        send({{"jsonrpc":"2.0","method":"turn/started","params":{{"threadId":thread_id,"turnId":"turn_1"}}}})
        send({{"jsonrpc":"2.0","method":"turn/completed","params":{{"threadId":thread_id,"turnId":"turn_1","status":"completed"}}}})
        continue
    if method == "turn/interrupt":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{}}}})
        continue
    if method == "thread/list":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"threads":[{{"id":"thr_test"}}],"nextCursor":None}}}})
        continue
    if method == "thread/read":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"thread":{{"id":"thr_test","turns":[]}}}}}})
        continue
    if method == "model/list":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"data":[{{"id":"gpt-5","displayName":"gpt-5"}}]}}}})
        continue
    if method == "thread/review":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"threadId":"thr_test"}}}})
        continue
    if method == "thread/compact/start":
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{}}}})
        continue
    if method == "approval/test":
        send({{"id":9001,"method":"execApproval","params":{{"threadId":"thr_test","command":["cargo","test","token=secret-value"]}}}})
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"queued":True}}}})
        continue
    if method == "approval/string":
        send({{"id":"req-string","method":"execApproval","params":{{"threadId":"thr_test","command":["cargo","test"]}}}})
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"queued":True}}}})
        continue
    if method == "secret/event":
        send({{"jsonrpc":"2.0","method":"turn/log","params":{{"threadId":"thr_test","message":"Authorization: Bearer secret-background-token token=secret-value","api_key":"plain-api-key"}}}})
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"queued":True}}}})
        continue
    if method == "collision/test":
        send({{"id":message.get("id"),"method":"execApproval","params":{{"threadId":"thr_test","command":["cargo","test"]}}}})
        send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{"queued":True}}}})
        continue
    send({{"jsonrpc":"2.0","id":message.get("id"),"result":{{}}}})
        "#,
    );

    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "codex2gpt-{}-{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse_log_entries(log: &Path) -> Vec<Value> {
    let mut entries = Vec::new();
    let raw = fs::read_to_string(log).unwrap();
    for line in raw.lines() {
        entries.push(serde_json::from_str(line).unwrap());
    }
    entries
}

fn wait_for_thread_events(client: &AppServerClient, thread_id: &str) -> Vec<Value> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        let events = client.events_for_thread(thread_id).unwrap();
        if !events.is_empty() {
            return events;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    client.events_for_thread(thread_id).unwrap()
}

#[test]
#[cfg(unix)]
fn appserver_client_initializes_once_and_runs_methods() {
    let state_dir = unique_temp_dir("appserver");
    let binary = state_dir.join("fake-appserver");
    write_fake_appserver(&binary);
    let log = state_dir.join("messages.jsonl");

    let client = AppServerClient::new(binary, state_dir.clone());
    let thread = client
        .call(
            "thread/start",
            json!({"cwd":"/tmp","input":[{"type":"text","text":"hello"}]}),
        )
        .unwrap();

    assert_eq!(thread["thread"]["id"], "thr_test");

    let entries = parse_log_entries(&log);
    assert!(entries.iter().any(|entry| entry["method"] == "initialize"));
    assert!(entries.iter().all(|entry| entry.get("jsonrpc").is_none()));
    assert!(
        entries.iter().any(|entry| entry["method"] == "thread/start"
            && entry["params"]["input"][0]["text"] == "hello")
    );
}

#[test]
#[cfg(unix)]
fn appserver_events_for_thread_are_persisted() {
    let state_dir = unique_temp_dir("appserver-events");
    let binary = state_dir.join("fake-appserver");
    write_fake_appserver(&binary);

    let client = AppServerClient::new(binary, state_dir.clone());
    client
        .call(
            "thread/start",
            json!({"cwd":"/tmp","input":[{"type":"text","text":"hello"}]}),
        )
        .unwrap();

    let events = wait_for_thread_events(&client, "thr_test");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["method"], "thread/started");
}

#[test]
#[cfg(unix)]
fn appserver_turn_interrupt_and_read_result() {
    let state_dir = unique_temp_dir("appserver-turn");
    let binary = state_dir.join("fake-appserver");
    write_fake_appserver(&binary);

    let client = AppServerClient::new(binary, state_dir.clone());
    let turn = client
        .call(
            "turn/start",
            json!({"threadId":"thr_test","input":[{"type":"text","text":"build"}]}),
        )
        .unwrap();
    assert_eq!(turn["turn"]["id"], "turn_1");

    let events = wait_for_thread_events(&client, "thr_test");
    assert!(events.iter().any(|event| event["method"] == "turn/started"));

    let interrupted = client
        .call(
            "turn/interrupt",
            json!({"threadId":"thr_test","turnId":"turn_1"}),
        )
        .unwrap();
    assert!(interrupted.as_object().is_some());
}

#[test]
#[cfg(unix)]
fn appserver_server_requests_are_stored_and_can_be_answered() {
    let state_dir = unique_temp_dir("appserver-approval");
    let binary = state_dir.join("fake-appserver");
    write_fake_appserver(&binary);
    let log = state_dir.join("messages.jsonl");

    let client = AppServerClient::new(binary, state_dir.clone());
    client.call("approval/test", json!({})).unwrap();

    let pending = wait_for_pending_requests(&client);
    assert_eq!(pending[0]["id"], 9001);
    assert_eq!(pending[0]["method"], "execApproval");
    assert_eq!(
        pending[0]["params"]["command"],
        json!(["cargo", "test", "token=[REDACTED]"])
    );

    client
        .respond(9001, json!({"decision":"deny","reason":"not approved"}))
        .unwrap();

    let entries = wait_for_log_entry(&log, |entry| {
        entry["id"] == 9001
            && entry.get("method").is_none()
            && entry["result"]["decision"] == "deny"
    });
    assert!(entries.iter().any(|entry| {
        entry["id"] == 9001
            && entry.get("method").is_none()
            && entry["result"]["decision"] == "deny"
    }));
    assert!(client.pending_requests().unwrap().is_empty());
}

#[test]
#[cfg(unix)]
fn appserver_server_requests_do_not_satisfy_waiters_with_colliding_ids() {
    let state_dir = unique_temp_dir("appserver-collision");
    let binary = state_dir.join("fake-appserver");
    write_fake_appserver(&binary);

    let client = AppServerClient::new(binary, state_dir.clone());
    let response = client.call("collision/test", json!({})).unwrap();

    assert_eq!(response["queued"], true);
    let pending = wait_for_pending_requests(&client);
    assert_eq!(pending[0]["method"], "execApproval");
}

fn wait_for_pending_requests(client: &AppServerClient) -> Vec<Value> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        let pending = client.pending_requests().unwrap();
        if !pending.is_empty() {
            return pending;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    client.pending_requests().unwrap()
}

fn wait_for_log_entry(log: &Path, predicate: impl Fn(&Value) -> bool) -> Vec<Value> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        let entries = parse_log_entries(log);
        if entries.iter().any(&predicate) {
            return entries;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    parse_log_entries(log)
}

#[test]
#[cfg(unix)]
fn appserver_event_log_redacts_secrets() {
    let state_dir = unique_temp_dir("appserver-secret-event");
    let binary = state_dir.join("fake-appserver");
    write_fake_appserver(&binary);

    let client = AppServerClient::new(binary, state_dir.clone());
    client.call("secret/event", json!({})).unwrap();
    let events = wait_for_thread_events(&client, "thr_test");

    let visible = serde_json::to_string(&events).unwrap();
    assert!(visible.contains("[REDACTED]"));
    assert!(!visible.contains("secret-background-token"));
    assert!(!visible.contains("secret-value"));
    assert!(!visible.contains("plain-api-key"));
}

#[test]
#[cfg(unix)]
fn appserver_reinitializes_after_process_restart() {
    let state_dir = unique_temp_dir("appserver-restart");
    let binary = state_dir.join("fake-appserver");
    write_restart_fake_appserver(&binary);
    let log = state_dir.join("messages.jsonl");

    let client = AppServerClient::new(binary, state_dir.clone());
    client.call("thread/list", json!({})).unwrap();
    client.call("thread/list", json!({})).unwrap();

    let initialize_count = parse_log_entries(&log)
        .iter()
        .filter(|entry| entry["method"] == "initialize")
        .count();
    assert_eq!(initialize_count, 2);
}

#[cfg(unix)]
fn write_restart_fake_appserver(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(
        path,
        r#"#!/usr/bin/env python3
import json
import os
import sys

log = os.path.join(os.path.dirname(sys.argv[0]), "messages.jsonl")
initialized = False

def log_input(data):
    with open(log, "a", encoding="utf-8") as out:
        out.write(json.dumps(data))
        out.write("\n")

def send(obj):
    print(json.dumps(obj), flush=True)

for raw in sys.stdin:
    if not raw.strip():
        continue
    message = json.loads(raw)
    log_input(message)
    method = message.get("method")
    if method == "initialize":
        initialized = True
        send({"id": message.get("id"), "result": {"serverInfo": {"name": "fake"}}})
    elif method == "initialized":
        pass
    elif method == "thread/list" and initialized:
        send({"id": message.get("id"), "result": {"threads": []}})
        sys.exit(0)
    else:
        send({"id": message.get("id"), "error": {"message": "not initialized"}})
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}
