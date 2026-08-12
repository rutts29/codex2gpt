use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AppError, Result};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub id: String,
    pub path: PathBuf,
    #[serde(default)]
    pub allow_write: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolSurface {
    Full,
    Advisor,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    pub state_dir: PathBuf,
    #[serde(default = "default_codex_binary")]
    pub codex_binary: String,
    #[serde(default = "default_git_binary")]
    pub git_binary: String,
    #[serde(default = "default_rg_binary")]
    pub rg_binary: String,
    #[serde(default = "default_max_read_bytes")]
    pub max_read_bytes: usize,
    #[serde(default = "default_max_search_results")]
    pub max_search_results: usize,
    #[serde(default)]
    pub widget_domain: Option<String>,
    #[serde(default = "default_tool_surface")]
    pub tool_surface: ToolSurface,
    #[serde(default)]
    pub allowed_workspaces: Vec<WorkspaceConfig>,
}

impl AppConfig {
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path).map_err(|source| AppError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        let config: Self = serde_json::from_str(&raw).map_err(|source| AppError::ParseJson {
            path: path.to_path_buf(),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn workspace(&self, id: &str) -> Result<&WorkspaceConfig> {
        self.allowed_workspaces
            .iter()
            .find(|workspace| workspace.id == id)
            .ok_or_else(|| AppError::UnknownWorkspace(id.to_owned()))
    }

    fn validate(&self) -> Result<()> {
        if !is_loopback_listen_addr(&self.listen_addr) {
            return Err(AppError::InvalidConfig(
                "listen_addr must bind to 127.0.0.1, localhost, or [::1]".to_owned(),
            ));
        }
        if let Some(widget_domain) = &self.widget_domain
            && !is_https_origin(widget_domain)
        {
            return Err(AppError::InvalidConfig(
                "widget_domain must be an HTTPS origin without path, query, fragment, or userinfo"
                    .to_owned(),
            ));
        }
        let mut seen = HashSet::new();
        for workspace in &self.allowed_workspaces {
            if !seen.insert(workspace.id.clone()) {
                return Err(AppError::DuplicateWorkspaceId(workspace.id.clone()));
            }
        }
        Ok(())
    }
}

fn is_loopback_listen_addr(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    let valid_host = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
    valid_host && !port.is_empty() && port.parse::<u16>().is_ok_and(|port| port > 0)
}

fn is_https_origin(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    if rest.is_empty()
        || rest.contains('/')
        || rest.contains('?')
        || rest.contains('#')
        || rest.contains('@')
    {
        return false;
    }

    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (rest, None),
    };
    if host.is_empty() || !host.split('.').all(is_valid_host_label) {
        return false;
    }
    if let Some(port) = port {
        let Ok(port) = port.parse::<u16>() else {
            return false;
        };
        if port == 0 {
            return false;
        }
    }

    true
}

fn is_valid_host_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !label.starts_with('-')
        && !label.ends_with('-')
}

fn default_listen_addr() -> String {
    "127.0.0.1:8787".to_owned()
}

fn default_codex_binary() -> String {
    "codex".to_owned()
}

fn default_git_binary() -> String {
    "git".to_owned()
}

fn default_rg_binary() -> String {
    "rg".to_owned()
}

fn default_max_read_bytes() -> usize {
    64 * 1024
}

fn default_max_search_results() -> usize {
    100
}

fn default_tool_surface() -> ToolSurface {
    ToolSurface::Full
}
