use std::fs;

use codex2gpt::runs::{RunState, RunStore};

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
fn create_run_writes_request_and_status_files() {
    let dir = unique_temp_dir("run-store");
    let store = RunStore::new(dir.clone());

    let status = store
        .create_run("repo", "inspect auth", RunState::Running)
        .unwrap();

    let run_dir = dir.join(&status.run_id);
    let request_json = fs::read_to_string(run_dir.join("request.json")).unwrap();
    let status_json = fs::read_to_string(run_dir.join("status.json")).unwrap();

    assert_eq!(status.workspace_id, "repo");
    assert_eq!(status.state, RunState::Running);
    assert!(request_json.contains("inspect auth"));
    assert!(status_json.contains(&status.run_id));
}

#[test]
fn fail_run_writes_failed_status_with_message() {
    let store = RunStore::new(unique_temp_dir("run-store-fail"));
    let status = store
        .create_run("repo", "inspect", RunState::Running)
        .unwrap();

    let failed = store
        .fail_run(&status.run_id, "codex exited with failure".to_owned())
        .unwrap();
    let loaded = store.load_status(&status.run_id).unwrap();

    assert_eq!(failed.state, RunState::Failed);
    assert_eq!(
        failed.final_message,
        Some("codex exited with failure".to_owned())
    );
    assert_eq!(loaded, failed);
}

#[test]
fn link_thread_writes_thread_id_without_marking_complete() {
    let store = RunStore::new(unique_temp_dir("run-store-link-thread"));
    let status = store
        .create_run("repo", "start app-server thread", RunState::Running)
        .unwrap();

    let linked = store.link_thread(&status.run_id, "thr_started").unwrap();
    let loaded = store.load_status(&status.run_id).unwrap();

    assert_eq!(linked.state, RunState::Running);
    assert_eq!(linked.thread_id, Some("thr_started".to_owned()));
    assert_eq!(loaded, linked);
}

#[test]
fn load_status_rejects_run_id_path_traversal() {
    let dir = unique_temp_dir("run-store-traversal");
    let store = RunStore::new(dir);

    let err = store.load_status("../outside").unwrap_err();

    assert!(err.to_string().contains("invalid run id"));
}
