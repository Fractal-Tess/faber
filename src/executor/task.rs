use super::error::ExecutionTaskError;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct TaskResult {
    pub status: TaskStatus,
    pub error: Option<ExecutionTaskError>,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum TaskStatus {
    Success,
    Failure,
    NotExecuted,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Success => write!(f, "success"),
            TaskStatus::Failure => write!(f, "failure"),
            TaskStatus::NotExecuted => write!(f, "not_executed"),
        }
    }
}

// Backward compatibility aliases
pub type ExecutionTask = Task;
pub type ExecutionTaskResult = TaskResult;
pub type ExecutionTaskStatus = TaskStatus;
