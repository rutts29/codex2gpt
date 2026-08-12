use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};
use std::collections::HashSet;

use crate::config::AppConfig;
use crate::context::{read_context, search_context};

pub(crate) fn search(
    config: &AppConfig,
    workspace_id: &str,
    query: &str,
) -> crate::error::Result<Value> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();
    let workspace = config.workspace(workspace_id)?;
    let search = search_context(config, &workspace.id, query)?;
    for matched in search.matches {
        let path = matched.path.trim_start_matches("./");
        if !seen.insert(path.to_owned()) {
            continue;
        }
        let id = encode_document_id(&workspace.id, path);
        results.push(json!({
            "id": id,
            "title": path,
            "url": format!("https://codex2gpt.local/document/{id}")
        }));
        if results.len() >= config.max_search_results {
            return Ok(json!({ "results": results }));
        }
    }

    Ok(json!({ "results": results }))
}

pub(crate) fn fetch(config: &AppConfig, document_id: &str) -> crate::error::Result<Value> {
    let (workspace, path) = decode_document_id(document_id)?;
    let file = read_context(config, &workspace, std::path::Path::new(&path))?;

    Ok(json!({
        "id": document_id,
        "title": path,
        "text": file.text,
        "url": format!("https://codex2gpt.local/document/{}", encode_document_id(&workspace, &path)),
        "metadata": {
            "workspace": workspace,
            "path": file.path,
            "truncated": file.truncated
        }
    }))
}

fn encode_document_id(workspace: &str, path: &str) -> String {
    format!("{workspace}/{path}")
}

fn decode_document_id(document_id: &str) -> crate::error::Result<(String, String)> {
    let document_id = document_id
        .strip_prefix("https://codex2gpt.local/document/")
        .or_else(|| document_id.strip_prefix("https://codex2gpt.local/fetch/"))
        .or_else(|| document_id.strip_prefix("codex2gpt://document/"))
        .or_else(|| document_id.strip_prefix("codex2gpt://"))
        .unwrap_or(document_id);
    if let Some((workspace, path)) = document_id.split_once('/')
        && !workspace.is_empty()
        && !path.is_empty()
    {
        return Ok((workspace.to_owned(), path.to_owned()));
    }
    let encoded = document_id
        .strip_prefix("doc_")
        .or_else(|| document_id.strip_prefix("v1:"))
        .ok_or_else(|| crate::error::AppError::SearchCommand("invalid document id".to_owned()))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| crate::error::AppError::SearchCommand("invalid document id".to_owned()))?;
    let decoded = String::from_utf8(decoded)
        .map_err(|_| crate::error::AppError::SearchCommand("invalid document id".to_owned()))?;
    let Some((workspace, path)) = decoded.split_once('\0') else {
        return Err(crate::error::AppError::SearchCommand(
            "invalid document id".to_owned(),
        ));
    };

    Ok((workspace.to_owned(), path.to_owned()))
}
