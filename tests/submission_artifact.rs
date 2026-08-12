use std::collections::HashSet;
use std::fs;

use codex2gpt::config::AppConfig;
use codex2gpt::mcp::{AppState, handle_json_rpc};
use serde_json::{Value, json};

fn state_with_workspace() -> AppState {
    let root = unique_temp_dir("submission-root");
    let state_dir = unique_temp_dir("submission-state");
    let config_path = state_dir.join("config.json");
    fs::write(
        &config_path,
        format!(
            r#"{{
              "state_dir": "{}",
              "allowed_workspaces": [
                {{"id": "repo", "path": "{}"}}
              ]
            }}"#,
            state_dir.display(),
            root.display()
        ),
    )
    .unwrap();
    AppState::new(AppConfig::load_from_file(&config_path).unwrap())
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
fn submission_artifact_covers_every_declared_tool() {
    let artifact: Value =
        serde_json::from_str(include_str!("../chatgpt-app-submission.json")).unwrap();
    let response = handle_json_rpc(
        &state_with_workspace(),
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
    );
    let descriptor_names: HashSet<_> = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_owned())
        .collect();
    let artifact_names: HashSet<_> = artifact["tools"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();

    assert_eq!(artifact_names, descriptor_names);
    let descriptor_annotations = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| (tool["name"].as_str().unwrap(), tool["annotations"].clone()))
        .collect::<std::collections::HashMap<_, _>>();
    for (name, artifact_tool) in artifact["tools"].as_object().unwrap() {
        assert_eq!(
            artifact_tool["annotations"],
            descriptor_annotations[name.as_str()],
            "submission annotations differ from runtime descriptor for {name}"
        );
    }

    for case_group in ["test_cases", "negative_test_cases"] {
        for case in artifact[case_group].as_array().unwrap() {
            let Some(triggered) = case["tools_triggered"].as_str() else {
                continue;
            };
            let tool_names: Vec<_> = triggered.split(',').map(str::trim).collect();
            if tool_names
                .iter()
                .all(|tool_name| is_tool_name_token(tool_name))
            {
                for tool_name in tool_names {
                    assert!(
                        artifact_names.contains(tool_name),
                        "{case_group} references undeclared tool: {tool_name}"
                    );
                }
            } else {
                assert!(
                    triggered.starts_with("No MCP tool."),
                    "{case_group} uses unvalidated tools_triggered prose: {triggered}"
                );
            }
        }
    }

    let test_descriptions: HashSet<_> = artifact["test_cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["description"].as_str().unwrap())
        .collect();
    for required in [
        "Start and steer an app-server-backed Codex thread.",
        "Resume, read, and export a Codex thread result bundle.",
        "Deny a pending Codex approval without allowing model-controlled approval.",
        "Complete ChatGPT OAuth discovery and resource-bound authorization.",
    ] {
        assert!(
            test_descriptions.contains(required),
            "missing submission test case: {required}"
        );
    }

    let negative_descriptions: HashSet<_> = artifact["negative_test_cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["description"].as_str().unwrap())
        .collect();
    for required in [
        "Reject unscoped local search.",
        "Reject model-controlled approval attempts.",
        "Reject OAuth authorization for a mismatched resource.",
    ] {
        assert!(
            negative_descriptions.contains(required),
            "missing negative submission test case: {required}"
        );
    }
}

fn is_tool_name_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
