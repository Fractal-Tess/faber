use super::error::ExecutionTaskError;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTask {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct ExecutionTaskResult {
    pub status: ExecutionTaskStatus,
    pub error: Option<ExecutionTaskError>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum ExecutionTaskStatus {
    Success,
    Failure,
    NotExecuted,
}

impl fmt::Display for ExecutionTaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionTaskStatus::Success => write!(f, "success"),
            ExecutionTaskStatus::Failure => write!(f, "failure"),
            ExecutionTaskStatus::NotExecuted => write!(f, "executed"),
        }
    }
}
