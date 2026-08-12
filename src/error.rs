use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("failed to read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse JSON in {path}: {source}")]
    ParseJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("duplicate workspace id: {0}")]
    DuplicateWorkspaceId(String),
    #[error("workspace id not found: {0}")]
    UnknownWorkspace(String),
    #[error("path escapes workspace: {0}")]
    WorkspaceEscape(PathBuf),
    #[error("symlink path rejected: {0}")]
    SymlinkRejected(PathBuf),
    #[error("path is not valid UTF-8")]
    InvalidPath,
    #[error("binary file rejected: {0}")]
    BinaryFile(PathBuf),
    #[error("codex command failed: {0}")]
    CodexCommand(String),
    #[error("search command failed: {0}")]
    SearchCommand(String),
    #[error("worktree command failed: {0}")]
    WorktreeCommand(String),
    #[error("failed to write {path}: {source}")]
    WriteFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid run id: {0}")]
    InvalidRunId(String),
    #[error("invalid worktree name: {0}")]
    InvalidWorktreeName(String),
    #[error("invalid git ref: {0}")]
    InvalidGitRef(String),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("workspace does not allow writes: {0}")]
    WorkspaceReadOnly(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
