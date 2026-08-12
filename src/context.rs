use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

use serde::Serialize;
use serde_json::Value;

use crate::audit::redact_for_log;
use crate::config::AppConfig;
use crate::error::{AppError, Result};
use crate::paths::resolve_workspace_path;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RepoBrief {
    pub workspace_id: String,
    pub root: String,
    pub has_git_dir: bool,
    pub entries: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ContextFile {
    pub path: String,
    pub text: String,
    pub truncated: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SearchResults {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub truncated: bool,
}

pub fn repo_brief(config: &AppConfig, workspace_id: &str) -> Result<RepoBrief> {
    let workspace = config.workspace(workspace_id)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&workspace.path).map_err(|source| AppError::ReadFile {
        path: workspace.path.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| AppError::ReadFile {
            path: workspace.path.clone(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let suffix = if entry.path().is_dir() { "/" } else { "" };
        entries.push(format!("{name}{suffix}"));
    }
    entries.sort();

    Ok(RepoBrief {
        workspace_id: workspace.id.clone(),
        root: workspace.path.display().to_string(),
        has_git_dir: workspace.path.join(".git").exists(),
        entries,
    })
}

pub fn read_context(
    config: &AppConfig,
    workspace_id: &str,
    relative: &Path,
) -> Result<ContextFile> {
    let workspace = config.workspace(workspace_id)?;
    let path = resolve_workspace_path(&workspace.path, relative)?;
    let mut file = fs::File::open(&path).map_err(|source| AppError::ReadFile {
        path: path.clone(),
        source,
    })?;
    let mut bytes = Vec::with_capacity(config.max_read_bytes.saturating_add(1));
    file.by_ref()
        .take(config.max_read_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| AppError::ReadFile {
            path: path.clone(),
            source,
        })?;
    if bytes.contains(&0) {
        return Err(AppError::BinaryFile(relative.to_path_buf()));
    }

    let truncated = bytes.len() > config.max_read_bytes;
    let capped = &bytes[..bytes.len().min(config.max_read_bytes)];
    let text = String::from_utf8_lossy(capped).to_string();

    Ok(ContextFile {
        path: relative.display().to_string(),
        text,
        truncated,
    })
}

pub fn search_context(
    config: &AppConfig,
    workspace_id: &str,
    query: &str,
) -> Result<SearchResults> {
    let workspace = config.workspace(workspace_id)?;
    let max_columns = config.max_read_bytes.max(1).to_string();
    let mut child = Command::new(&config.rg_binary)
        .arg("--json")
        .arg("--fixed-strings")
        .arg("--color")
        .arg("never")
        .arg("--no-heading")
        .arg("--max-columns")
        .arg(&max_columns)
        .arg("--")
        .arg(query)
        .arg(".")
        .current_dir(&workspace.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| AppError::SearchCommand(source.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::SearchCommand("missing rg stdout".to_owned()))?;
    let mut matches = Vec::new();
    let mut truncated = false;

    let output_budget = config
        .max_read_bytes
        .max(1)
        .saturating_mul(config.max_search_results.saturating_add(1).max(1));
    let mut reader = BufReader::new(stdout).take(output_budget as u64);
    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|source| AppError::SearchCommand(source.to_string()))?;
        if bytes == 0 {
            break;
        }
        if reader.limit() == 0 && !line.ends_with('\n') {
            truncated = true;
            let _ = child.kill();
            break;
        }
        let Some(search_match) = parse_rg_match(&line)? else {
            continue;
        };
        if matches.len() >= config.max_search_results {
            truncated = true;
            let _ = child.kill();
            break;
        }
        matches.push(search_match);
    }

    let output = child
        .wait_with_output()
        .map_err(|source| AppError::SearchCommand(source.to_string()))?;
    let no_matches = output.status.code() == Some(1) && matches.is_empty();
    if !truncated && !output.status.success() && !no_matches {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::SearchCommand(redact_for_log(stderr.trim())));
    }

    Ok(SearchResults {
        query: query.to_owned(),
        matches,
        truncated,
    })
}

fn parse_rg_match(line: &str) -> Result<Option<SearchMatch>> {
    if line.trim().is_empty() {
        return Ok(None);
    }

    let value: Value = serde_json::from_str(line)
        .map_err(|source| AppError::SearchCommand(format!("invalid rg JSON: {source}")))?;
    if value.get("type").and_then(Value::as_str) != Some("match") {
        return Ok(None);
    }

    let data = value
        .get("data")
        .ok_or_else(|| AppError::SearchCommand("rg match missing data".to_owned()))?;
    let path = data
        .get("path")
        .and_then(|path| path.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::SearchCommand("rg match missing path".to_owned()))?;
    let line = data
        .get("line_number")
        .and_then(Value::as_u64)
        .ok_or_else(|| AppError::SearchCommand("rg match missing line number".to_owned()))?;
    let text = data
        .get("lines")
        .and_then(|lines| lines.get("text"))
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::SearchCommand("rg match missing line text".to_owned()))?;

    Ok(Some(SearchMatch {
        path: path.to_owned(),
        line: line as usize,
        text: text
            .trim_end_matches(|character| character == '\n' || character == '\r')
            .to_owned(),
    }))
}
