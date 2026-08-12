use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RunStatus {
    pub run_id: String,
    pub workspace_id: String,
    pub state: RunState,
    pub final_message: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RunRequestRecord {
    pub workspace_id: String,
    pub prompt: String,
}

#[derive(Clone, Debug)]
pub struct RunStore {
    root: PathBuf,
}

impl RunStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn create_run(
        &self,
        workspace_id: &str,
        prompt: &str,
        state: RunState,
    ) -> Result<RunStatus> {
        fs::create_dir_all(&self.root).map_err(|source| AppError::WriteFile {
            path: self.root.clone(),
            source,
        })?;
        let status = RunStatus {
            run_id: new_run_id(),
            workspace_id: workspace_id.to_owned(),
            state,
            final_message: None,
            thread_id: None,
        };
        let run_dir = self.root.join(&status.run_id);
        fs::create_dir(&run_dir).map_err(|source| AppError::WriteFile {
            path: run_dir.clone(),
            source,
        })?;
        write_json(
            run_dir.join("request.json"),
            &RunRequestRecord {
                workspace_id: workspace_id.to_owned(),
                prompt: prompt.to_owned(),
            },
        )?;
        write_json(run_dir.join("status.json"), &status)?;
        Ok(status)
    }

    pub fn load_status(&self, run_id: &str) -> Result<RunStatus> {
        validate_run_id(run_id)?;
        let path = self.root.join(run_id).join("status.json");
        let raw = fs::read_to_string(&path).map_err(|source| AppError::ReadFile {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&raw).map_err(|source| AppError::ParseJson { path, source })
    }

    pub fn complete_run(
        &self,
        run_id: &str,
        final_message: Option<String>,
        thread_id: Option<String>,
    ) -> Result<RunStatus> {
        let mut status = self.load_status(run_id)?;
        status.state = RunState::Completed;
        status.final_message = final_message;
        status.thread_id = thread_id;
        self.write_status(status)
    }

    pub fn link_thread(&self, run_id: &str, thread_id: &str) -> Result<RunStatus> {
        let mut status = self.load_status(run_id)?;
        status.thread_id = Some(thread_id.to_owned());
        self.write_status(status)
    }

    pub fn workspace_for_thread(&self, thread_id: &str) -> Result<Option<String>> {
        if !self.root.exists() {
            return Ok(None);
        }
        for entry in fs::read_dir(&self.root).map_err(|source| AppError::ReadFile {
            path: self.root.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| AppError::ReadFile {
                path: self.root.clone(),
                source,
            })?;
            let status_path = entry.path().join("status.json");
            if !status_path.exists() {
                continue;
            }
            let raw = fs::read_to_string(&status_path).map_err(|source| AppError::ReadFile {
                path: status_path.clone(),
                source,
            })?;
            let status: RunStatus =
                serde_json::from_str(&raw).map_err(|source| AppError::ParseJson {
                    path: status_path,
                    source,
                })?;
            if status.thread_id.as_deref() == Some(thread_id) {
                return Ok(Some(status.workspace_id));
            }
        }
        Ok(None)
    }

    pub fn fail_run(&self, run_id: &str, final_message: String) -> Result<RunStatus> {
        let mut status = self.load_status(run_id)?;
        status.state = RunState::Failed;
        status.final_message = Some(final_message);
        self.write_status(status)
    }

    fn write_status(&self, status: RunStatus) -> Result<RunStatus> {
        write_json(self.root.join(&status.run_id).join("status.json"), &status)?;
        Ok(status)
    }
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let data = serde_json::to_vec_pretty(value).expect("serializing run records cannot fail");
    fs::write(&path, data).map_err(|source| AppError::WriteFile { path, source })
}

fn validate_run_id(run_id: &str) -> Result<()> {
    if run_id.is_empty()
        || !run_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(AppError::InvalidRunId(run_id.to_owned()));
    }
    Ok(())
}

fn new_run_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before UNIX epoch")
        .as_nanos();
    format!("run-{}-{nanos}", std::process::id())
}
