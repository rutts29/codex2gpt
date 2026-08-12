use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::audit::redact_for_log;
use crate::config::AppConfig;
use crate::error::{AppError, Result};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct WorktreeList {
    pub workspace_id: String,
    pub worktrees: Vec<WorktreeInfo>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CreatedWorktree {
    pub workspace_id: String,
    pub name: String,
    pub branch: String,
    pub base: String,
    pub path: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RemovedWorktree {
    pub workspace_id: String,
    pub name: String,
    pub path: String,
}

pub fn list_worktrees(config: &AppConfig, workspace_id: &str) -> Result<WorktreeList> {
    let workspace = config.workspace(workspace_id)?;
    let output = Command::new(&config.git_binary)
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&workspace.path)
        .output()
        .map_err(|source| AppError::WorktreeCommand(source.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::WorktreeCommand(redact_for_log(stderr.trim())));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    Ok(WorktreeList {
        workspace_id: workspace.id.clone(),
        worktrees: parse_worktree_porcelain(&raw)
            .into_iter()
            .filter_map(|worktree| sanitize_worktree(worktree, &workspace.path))
            .collect(),
    })
}

pub fn create_worktree(
    config: &AppConfig,
    workspace_id: &str,
    name: &str,
    base: &str,
) -> Result<CreatedWorktree> {
    let workspace = config.workspace(workspace_id)?;
    if !workspace.allow_write {
        return Err(AppError::WorkspaceReadOnly(workspace.id.clone()));
    }
    validate_worktree_name(name)?;
    validate_git_ref(base)?;

    let (root, parent, destination) = managed_worktree_destination(config, workspace_id, name)?;
    let destination_parent = destination
        .parent()
        .ok_or_else(|| AppError::WorktreeCommand("worktree path has no parent".to_owned()))?;
    fs::create_dir_all(destination_parent).map_err(|source| AppError::WriteFile {
        path: destination_parent.to_path_buf(),
        source,
    })?;
    let branch = format!("codex2gpt/{name}");

    let output = Command::new(&config.git_binary)
        .arg("worktree")
        .arg("add")
        .arg("-b")
        .arg(&branch)
        .arg(&destination)
        .arg(base)
        .current_dir(&root)
        .output()
        .map_err(|source| AppError::WorktreeCommand(source.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::WorktreeCommand(redact_for_log(stderr.trim())));
    }

    Ok(CreatedWorktree {
        workspace_id: workspace.id.clone(),
        name: name.to_owned(),
        branch,
        base: base.to_owned(),
        path: destination
            .strip_prefix(parent)
            .unwrap_or(&destination)
            .display()
            .to_string(),
    })
}

pub fn remove_worktree(
    config: &AppConfig,
    workspace_id: &str,
    name: &str,
) -> Result<RemovedWorktree> {
    let workspace = config.workspace(workspace_id)?;
    if !workspace.allow_write {
        return Err(AppError::WorkspaceReadOnly(workspace.id.clone()));
    }
    validate_worktree_name(name)?;

    let (root, parent, destination) = managed_worktree_destination(config, workspace_id, name)?;

    let output = Command::new(&config.git_binary)
        .arg("worktree")
        .arg("remove")
        .arg(&destination)
        .current_dir(&root)
        .output()
        .map_err(|source| AppError::WorktreeCommand(source.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::WorktreeCommand(redact_for_log(stderr.trim())));
    }

    Ok(RemovedWorktree {
        workspace_id: workspace.id.clone(),
        name: name.to_owned(),
        path: destination
            .strip_prefix(parent)
            .unwrap_or(&destination)
            .display()
            .to_string(),
    })
}

pub fn managed_worktree_path(
    config: &AppConfig,
    workspace_id: &str,
    name: &str,
) -> Result<PathBuf> {
    managed_worktree_destination(config, workspace_id, name).map(|(_, _, destination)| destination)
}

fn managed_worktree_destination(
    config: &AppConfig,
    workspace_id: &str,
    name: &str,
) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let workspace = config.workspace(workspace_id)?;
    validate_worktree_name(name)?;
    let root = workspace
        .path
        .canonicalize()
        .map_err(|source| AppError::ReadFile {
            path: workspace.path.clone(),
            source,
        })?;
    let parent = root
        .parent()
        .ok_or_else(|| AppError::WorktreeCommand("workspace has no parent".to_owned()))?
        .to_path_buf();
    let destination = parent
        .join(".codex2gpt-worktrees")
        .join(&workspace.id)
        .join(name);
    Ok((root, parent, destination))
}

fn validate_worktree_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 80
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    if valid {
        Ok(())
    } else {
        Err(AppError::InvalidWorktreeName(name.to_owned()))
    }
}

fn validate_git_ref(git_ref: &str) -> Result<()> {
    let valid = !git_ref.is_empty()
        && git_ref.len() <= 160
        && !git_ref.starts_with('-')
        && !git_ref.contains("..")
        && !git_ref.contains("@{")
        && !git_ref.ends_with('/')
        && git_ref
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(AppError::InvalidGitRef(git_ref.to_owned()))
    }
}

fn sanitize_worktree(mut worktree: WorktreeInfo, workspace_root: &Path) -> Option<WorktreeInfo> {
    let root = workspace_root.canonicalize().ok()?;
    let parent = root.parent()?;
    let path = Path::new(&worktree.path).canonicalize().ok()?;
    let display = path.strip_prefix(parent).ok()?;
    worktree.path = display.display().to_string();
    Some(worktree)
}

fn parse_worktree_porcelain(raw: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current: Option<WorktreeInfo> = None;

    for line in raw.lines() {
        if line.is_empty() {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
            current = Some(WorktreeInfo {
                path: path.to_owned(),
                branch: None,
                commit: None,
            });
        } else if let Some(commit) = line.strip_prefix("HEAD ") {
            if let Some(worktree) = current.as_mut() {
                worktree.commit = Some(commit.to_owned());
            }
        } else if let Some(branch) = line.strip_prefix("branch ") {
            if let Some(worktree) = current.as_mut() {
                worktree.branch = Some(
                    branch
                        .strip_prefix("refs/heads/")
                        .unwrap_or(branch)
                        .to_owned(),
                );
            }
        }
    }

    if let Some(worktree) = current {
        worktrees.push(worktree);
    }

    worktrees
}
