use std::fs;
use std::path::Path;

use codex2gpt::config::{AppConfig, ToolSurface, WorkspaceConfig};
use codex2gpt::worktrees::{create_worktree, list_worktrees, remove_worktree};

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

fn config_with_git(root: &Path, git_binary: &Path) -> AppConfig {
    config_with_git_and_write(root, git_binary, false)
}

fn config_with_git_and_write(root: &Path, git_binary: &Path, allow_write: bool) -> AppConfig {
    AppConfig {
        listen_addr: "127.0.0.1:8787".to_owned(),
        state_dir: unique_temp_dir("worktree-state"),
        codex_binary: "codex".to_owned(),
        git_binary: git_binary.display().to_string(),
        rg_binary: "rg".to_owned(),
        max_read_bytes: 64 * 1024,
        max_search_results: 100,
        widget_domain: None,
        tool_surface: ToolSurface::Full,
        allowed_workspaces: vec![WorkspaceConfig {
            id: "repo".to_owned(),
            path: root.to_path_buf(),
            allow_write,
        }],
    }
}

#[cfg(unix)]
fn write_fake_git(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, format!("#!/bin/sh\ncat <<'EOF'\n{body}\nEOF\n")).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn write_recording_git(path: &Path, log_path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(
        path,
        format!(
            r#"#!/bin/sh
printf 'pwd=%s\n' "$PWD" > "{}"
printf 'args=%s\n' "$*" >> "{}"
exit 0
"#,
            log_path.display(),
            log_path.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
#[cfg(unix)]
fn list_worktrees_parses_porcelain_output() {
    let root = unique_temp_dir("worktree-root");
    let sibling = root.with_file_name(format!(
        "{}-feature",
        root.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&sibling).unwrap();
    let git = root.join("fake-git");
    write_fake_git(
        &git,
        &format!(
            r#"worktree {}
HEAD abc123
branch refs/heads/main

worktree {}
HEAD def456
branch refs/heads/feature/demo
"#,
            root.display(),
            sibling.display()
        ),
    );

    let worktrees = list_worktrees(&config_with_git(&root, &git), "repo").unwrap();

    assert_eq!(worktrees.workspace_id, "repo");
    assert_eq!(worktrees.worktrees.len(), 2);
    assert_eq!(
        worktrees.worktrees[0].path,
        root.file_name().unwrap().to_string_lossy()
    );
    assert_eq!(worktrees.worktrees[0].branch.as_deref(), Some("main"));
    assert_eq!(worktrees.worktrees[1].commit.as_deref(), Some("def456"));
    assert_eq!(
        worktrees.worktrees[1].branch.as_deref(),
        Some("feature/demo")
    );
    assert!(!worktrees.worktrees[1].path.starts_with('/'));
}

#[test]
#[cfg(unix)]
fn list_worktrees_rejects_unknown_workspace() {
    let root = unique_temp_dir("worktree-unknown-root");
    let git = root.join("fake-git");
    write_fake_git(&git, "");

    let err = list_worktrees(&config_with_git(&root, &git), "missing").unwrap_err();

    assert!(err.to_string().contains("workspace id not found"));
}

#[test]
#[cfg(unix)]
fn list_worktrees_omits_paths_outside_workspace_parent() {
    let parent = unique_temp_dir("worktree-filter-parent");
    let root = parent.join("repo");
    fs::create_dir_all(&root).unwrap();
    let outside_parent = unique_temp_dir("worktree-outside-parent");
    let outside = outside_parent.join("other");
    fs::create_dir_all(&outside).unwrap();
    let git = root.join("fake-git");
    write_fake_git(
        &git,
        &format!(
            r#"worktree {}
HEAD abc123
branch refs/heads/main

worktree {}
HEAD def456
branch refs/heads/other
"#,
            root.display(),
            outside.display()
        ),
    );

    let worktrees = list_worktrees(&config_with_git(&root, &git), "repo").unwrap();

    assert_eq!(worktrees.worktrees.len(), 1);
    assert_eq!(worktrees.worktrees[0].branch.as_deref(), Some("main"));
}

#[test]
#[cfg(unix)]
fn create_worktree_runs_git_add_in_managed_directory() {
    let parent = unique_temp_dir("worktree-create-parent");
    let root = parent.join("repo");
    fs::create_dir_all(&root).unwrap();
    let git = root.join("fake-git");
    let log = root.join("git.log");
    write_recording_git(&git, &log);

    let created = create_worktree(
        &config_with_git_and_write(&root, &git, true),
        "repo",
        "feature-a",
        "main",
    )
    .unwrap();

    let log = fs::read_to_string(log).unwrap();
    let canonical_root = root.canonicalize().unwrap();
    let managed_path = canonical_root
        .parent()
        .unwrap()
        .join(".codex2gpt-worktrees")
        .join("repo")
        .join("feature-a");

    assert_eq!(created.workspace_id, "repo");
    assert_eq!(created.name, "feature-a");
    assert_eq!(created.branch, "codex2gpt/feature-a");
    assert_eq!(created.base, "main");
    assert_eq!(created.path, ".codex2gpt-worktrees/repo/feature-a");
    assert!(log.contains(&format!("pwd={}", canonical_root.display())));
    assert!(log.contains(&format!(
        "args=worktree add -b codex2gpt/feature-a {} main",
        managed_path.display()
    )));
}

#[test]
#[cfg(unix)]
fn create_worktree_creates_managed_parent_directory() {
    let parent = unique_temp_dir("worktree-managed-parent");
    let root = parent.join("repo");
    fs::create_dir_all(&root).unwrap();
    let git = root.join("fake-git");
    let log = root.join("git.log");
    write_recording_git(&git, &log);

    create_worktree(
        &config_with_git_and_write(&root, &git, true),
        "repo",
        "feature-a",
        "main",
    )
    .unwrap();

    assert!(parent.join(".codex2gpt-worktrees").join("repo").is_dir());
}

#[test]
#[cfg(unix)]
fn create_worktree_rejects_read_only_workspaces() {
    let root = unique_temp_dir("worktree-readonly-root");
    let git = root.join("fake-git");
    write_fake_git(&git, "");

    let err =
        create_worktree(&config_with_git(&root, &git), "repo", "feature-a", "main").unwrap_err();

    assert!(err.to_string().contains("workspace does not allow writes"));
}

#[test]
#[cfg(unix)]
fn create_worktree_rejects_unsafe_names() {
    let root = unique_temp_dir("worktree-unsafe-root");
    let git = root.join("fake-git");
    write_fake_git(&git, "");

    let err = create_worktree(
        &config_with_git_and_write(&root, &git, true),
        "repo",
        "../feature-a",
        "main",
    )
    .unwrap_err();

    assert!(err.to_string().contains("invalid worktree name"));
}

#[test]
#[cfg(unix)]
fn create_worktree_rejects_unsafe_base_refs() {
    let root = unique_temp_dir("worktree-unsafe-base-root");
    let git = root.join("fake-git");
    write_fake_git(&git, "");

    let err = create_worktree(
        &config_with_git_and_write(&root, &git, true),
        "repo",
        "feature-a",
        "--upload-pack=bad",
    )
    .unwrap_err();

    assert!(err.to_string().contains("invalid git ref"));
}

#[test]
#[cfg(unix)]
fn remove_worktree_runs_git_remove_for_managed_directory() {
    let parent = unique_temp_dir("worktree-remove-parent");
    let root = parent.join("repo");
    fs::create_dir_all(&root).unwrap();
    let git = root.join("fake-git");
    let log = root.join("git.log");
    write_recording_git(&git, &log);

    let removed = remove_worktree(
        &config_with_git_and_write(&root, &git, true),
        "repo",
        "feature-a",
    )
    .unwrap();
    let log = fs::read_to_string(log).unwrap();
    let canonical_root = root.canonicalize().unwrap();
    let managed_path = canonical_root
        .parent()
        .unwrap()
        .join(".codex2gpt-worktrees")
        .join("repo")
        .join("feature-a");

    assert_eq!(removed.workspace_id, "repo");
    assert_eq!(removed.name, "feature-a");
    assert_eq!(removed.path, ".codex2gpt-worktrees/repo/feature-a");
    assert!(log.contains(&format!("pwd={}", canonical_root.display())));
    assert!(log.contains(&format!("args=worktree remove {}", managed_path.display())));
}

#[test]
#[cfg(unix)]
fn remove_worktree_rejects_read_only_workspaces() {
    let root = unique_temp_dir("worktree-remove-readonly-root");
    let git = root.join("fake-git");
    write_fake_git(&git, "");

    let err = remove_worktree(&config_with_git(&root, &git), "repo", "feature-a").unwrap_err();

    assert!(err.to_string().contains("workspace does not allow writes"));
}

#[test]
#[cfg(unix)]
fn remove_worktree_rejects_unsafe_names() {
    let root = unique_temp_dir("worktree-remove-unsafe-root");
    let git = root.join("fake-git");
    write_fake_git(&git, "");

    let err = remove_worktree(
        &config_with_git_and_write(&root, &git, true),
        "repo",
        "../feature-a",
    )
    .unwrap_err();

    assert!(err.to_string().contains("invalid worktree name"));
}
