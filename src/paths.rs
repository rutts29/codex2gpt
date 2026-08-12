use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, Result};

pub fn resolve_workspace_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(AppError::WorkspaceEscape(relative.to_path_buf()));
    }

    let root = root.canonicalize().map_err(|source| AppError::ReadFile {
        path: root.to_path_buf(),
        source,
    })?;
    reject_symlink_components(&root, relative)?;
    let candidate = root.join(relative);
    let existing_parent = existing_parent(&candidate);
    let canonical_parent = existing_parent
        .canonicalize()
        .map_err(|source| AppError::ReadFile {
            path: existing_parent.to_path_buf(),
            source,
        })?;

    if !canonical_parent.starts_with(&root) {
        return Err(AppError::WorkspaceEscape(relative.to_path_buf()));
    }

    Ok(candidate)
}

fn reject_symlink_components(root: &Path, relative: &Path) -> Result<()> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        current.push(part);
        if let Ok(metadata) = std::fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err(AppError::SymlinkRejected(relative.to_path_buf()));
            }
        }
    }
    Ok(())
}

fn existing_parent(path: &Path) -> &Path {
    let mut current = path;
    while !current.exists() {
        current = current.parent().unwrap_or(Path::new("."));
    }
    current
}
