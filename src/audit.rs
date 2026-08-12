use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::error::{AppError, Result};

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub kind: String,
    pub message: String,
}

impl AuditEvent {
    pub fn new(kind: impl Into<String>, message: impl AsRef<str>) -> Self {
        Self {
            kind: kind.into(),
            message: redact_for_log(message.as_ref()),
        }
    }
}

pub fn redact_for_log(input: &str) -> String {
    let mut redact_next = false;
    let mut parts = Vec::new();
    for part in input.split_whitespace() {
        if redact_next {
            parts.push("[REDACTED]".to_owned());
            redact_next = false;
            continue;
        }
        if part.eq_ignore_ascii_case("bearer") || part.eq_ignore_ascii_case("authorization:bearer")
        {
            redact_next = true;
            parts.push(part.to_owned());
        } else if part
            .to_ascii_lowercase()
            .starts_with("authorization:bearer")
        {
            parts.push("Authorization:Bearer [REDACTED]".to_owned());
        } else {
            parts.push(redact_part(part));
        }
    }
    parts.join(" ")
}

pub fn append_audit_event(state_dir: &Path, event: &AuditEvent) -> Result<()> {
    fs::create_dir_all(state_dir).map_err(|source| AppError::WriteFile {
        path: state_dir.to_path_buf(),
        source,
    })?;
    let path = state_dir.join("audit.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|source| AppError::WriteFile {
            path: path.clone(),
            source,
        })?;
    serde_json::to_writer(&mut file, event).map_err(|source| AppError::ParseJson {
        path: path.clone(),
        source,
    })?;
    file.write_all(b"\n")
        .map_err(|source| AppError::WriteFile { path, source })
}

fn redact_part(part: &str) -> String {
    let trimmed = part.trim_matches(|ch| matches!(ch, '"' | '\'' | ',' | ';'));
    if trimmed.starts_with("sk-") {
        "[REDACTED]".to_owned()
    } else if let Some((key, _)) = part.split_once('=') {
        let key = key.trim_matches(|ch| matches!(ch, '"' | '\''));
        if key.eq_ignore_ascii_case("token")
            || key.eq_ignore_ascii_case("authorization")
            || key.eq_ignore_ascii_case("api_key")
            || key.to_ascii_uppercase().ends_with("_API_KEY")
            || key.to_ascii_lowercase().contains("token")
            || key.to_ascii_lowercase().contains("secret")
        {
            format!("{key}=[REDACTED]")
        } else {
            part.to_owned()
        }
    } else if part.len() >= 12 && part.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        "[REDACTED]".to_owned()
    } else {
        part.to_owned()
    }
}
