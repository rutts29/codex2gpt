use std::fs;
use std::path::Path;

use codex2gpt::config::{AppConfig, ToolSurface, WorkspaceConfig};
use codex2gpt::context::{read_context, repo_brief, search_context};

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

fn config_for(root: &Path, max_read_bytes: usize) -> AppConfig {
    config_with_rg(root, max_read_bytes, "rg", 100)
}

fn config_with_rg(
    root: &Path,
    max_read_bytes: usize,
    rg_binary: &str,
    max_search_results: usize,
) -> AppConfig {
    AppConfig {
        listen_addr: "127.0.0.1:8787".to_owned(),
        state_dir: unique_temp_dir("context-state"),
        codex_binary: "codex".to_owned(),
        git_binary: "git".to_owned(),
        rg_binary: rg_binary.to_owned(),
        max_read_bytes,
        max_search_results,
        widget_domain: None,
        tool_surface: ToolSurface::Full,
        allowed_workspaces: vec![WorkspaceConfig {
            id: "repo".to_owned(),
            path: root.to_path_buf(),
            allow_write: false,
        }],
    }
}

#[cfg(unix)]
fn write_fake_rg(path: &Path, body: &str, exit_code: u8) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(
        path,
        format!("#!/bin/sh\ncat <<'EOF'\n{body}\nEOF\nexit {exit_code}\n"),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn write_arg_recording_rg(path: &Path, args_path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(
        path,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$@" > '{}'
exit 1
"#,
            args_path.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn repo_brief_reports_top_level_files_and_git_presence() {
    let root = unique_temp_dir("brief-root");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n").unwrap();
    fs::write(root.join("README.md"), "# Demo\n").unwrap();
    fs::create_dir(root.join(".git")).unwrap();
    fs::create_dir(root.join("src")).unwrap();

    let brief = repo_brief(&config_for(&root, 1024), "repo").unwrap();

    assert_eq!(brief.workspace_id, "repo");
    assert!(brief.has_git_dir);
    assert!(brief.entries.contains(&"Cargo.toml".to_owned()));
    assert!(brief.entries.contains(&"README.md".to_owned()));
    assert!(brief.entries.contains(&"src/".to_owned()));
}

#[test]
fn read_context_returns_text_file_contents() {
    let root = unique_temp_dir("read-root");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();

    let file = read_context(&config_for(&root, 1024), "repo", Path::new("src/lib.rs")).unwrap();

    assert_eq!(file.path, "src/lib.rs");
    assert_eq!(file.text, "pub fn demo() {}\n");
    assert!(!file.truncated);
}

#[test]
fn read_context_truncates_to_configured_limit() {
    let root = unique_temp_dir("truncated-root");
    fs::write(root.join("big.txt"), "abcdef").unwrap();

    let file = read_context(&config_for(&root, 3), "repo", Path::new("big.txt")).unwrap();

    assert_eq!(file.text, "abc");
    assert!(file.truncated);
}

#[test]
fn read_context_rejects_binary_files() {
    let root = unique_temp_dir("binary-root");
    fs::write(root.join("data.bin"), b"abc\0def").unwrap();

    let err = read_context(&config_for(&root, 1024), "repo", Path::new("data.bin")).unwrap_err();

    assert!(err.to_string().contains("binary"));
}

#[test]
fn read_context_rejects_path_escape() {
    let root = unique_temp_dir("read-escape-root");

    let err = read_context(&config_for(&root, 1024), "repo", Path::new("../secret")).unwrap_err();

    assert!(err.to_string().contains("escapes workspace"));
}

#[test]
fn read_context_does_not_require_reading_past_limit() {
    let root = unique_temp_dir("bounded-read-root");
    fs::write(root.join("large.txt"), "abcdef").unwrap();

    let file = read_context(&config_for(&root, 1), "repo", Path::new("large.txt")).unwrap();

    assert_eq!(file.text, "a");
    assert!(file.truncated);
}

#[test]
#[cfg(unix)]
fn search_context_returns_bounded_rg_matches() {
    let root = unique_temp_dir("search-root");
    let rg = root.join("fake-rg");
    write_fake_rg(
        &rg,
        r#"{"type":"match","data":{"path":{"text":"src/lib.rs"},"lines":{"text":"fn bridge() {}\n"},"line_number":7}}
{"type":"match","data":{"path":{"text":"README.md"},"lines":{"text":"bridge notes\n"},"line_number":1}}"#,
        0,
    );

    let results = search_context(
        &config_with_rg(&root, 1024, rg.to_str().unwrap(), 10),
        "repo",
        "bridge",
    )
    .unwrap();

    assert_eq!(results.query, "bridge");
    assert_eq!(results.matches.len(), 2);
    assert_eq!(results.matches[0].path, "src/lib.rs");
    assert_eq!(results.matches[0].line, 7);
    assert_eq!(results.matches[0].text, "fn bridge() {}");
    assert!(!results.truncated);
}

#[test]
#[cfg(unix)]
fn search_context_separates_flag_like_queries_from_rg_options() {
    let root = unique_temp_dir("search-flag-root");
    let rg = root.join("fake-rg");
    let args_path = root.join("rg-args.txt");
    write_arg_recording_rg(&rg, &args_path);

    let results = search_context(
        &config_with_rg(&root, 1024, rg.to_str().unwrap(), 10),
        "repo",
        "--files",
    )
    .unwrap();

    let args = fs::read_to_string(args_path).unwrap();
    let lines = args.lines().collect::<Vec<_>>();
    assert!(results.matches.is_empty());
    assert!(
        lines
            .windows(3)
            .any(|window| window == ["--", "--files", "."])
    );
}

#[test]
#[cfg(unix)]
fn search_context_treats_rg_no_matches_as_empty_results() {
    let root = unique_temp_dir("search-empty-root");
    let rg = root.join("fake-rg");
    write_fake_rg(&rg, "", 1);

    let results = search_context(
        &config_with_rg(&root, 1024, rg.to_str().unwrap(), 10),
        "repo",
        "missing",
    )
    .unwrap();

    assert!(results.matches.is_empty());
    assert!(!results.truncated);
}

#[test]
#[cfg(unix)]
fn search_context_truncates_to_configured_result_limit() {
    let root = unique_temp_dir("search-truncate-root");
    let rg = root.join("fake-rg");
    write_fake_rg(
        &rg,
        r#"{"type":"match","data":{"path":{"text":"a.txt"},"lines":{"text":"one\n"},"line_number":1}}
{"type":"match","data":{"path":{"text":"b.txt"},"lines":{"text":"two\n"},"line_number":2}}"#,
        0,
    );

    let results = search_context(
        &config_with_rg(&root, 1024, rg.to_str().unwrap(), 1),
        "repo",
        "needle",
    )
    .unwrap();

    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].path, "a.txt");
    assert!(results.truncated);
}

#[test]
#[cfg(unix)]
fn search_context_truncates_when_rg_output_exceeds_byte_budget() {
    let root = unique_temp_dir("search-byte-budget-root");
    let rg = root.join("fake-rg");
    write_fake_rg(
        &rg,
        r#"{"type":"match","data":{"path":{"text":"huge.txt"},"lines":{"text":"this line is intentionally too large for the configured budget\n"},"line_number":1}}"#,
        0,
    );

    let results = search_context(
        &config_with_rg(&root, 8, rg.to_str().unwrap(), 10),
        "repo",
        "large",
    )
    .unwrap();

    assert!(results.matches.is_empty());
    assert!(results.truncated);
}
