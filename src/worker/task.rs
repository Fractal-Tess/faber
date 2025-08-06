use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// A task to be executed by a worker
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Task {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
}

/// The result of a task execution
#[derive(Debug, Clone, Serialize)]
#[serde(crate = "serde")]
pub struct TaskResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration: Duration,
    pub error: Option<String>,
}

impl TaskResult {
    pub fn new() -> Self {
        Self {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: -1,
            duration: Duration::ZERO,
            error: None,
        }
    }

    pub fn success(stdout: String, stderr: String, exit_code: i32, duration: Duration) -> Self {
        Self {
            success: true,
            stdout,
            stderr,
            exit_code,
            duration,
            error: None,
        }
    }

    pub fn failure(error: String, duration: Duration) -> Self {
        Self {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: -1,
            duration,
            error: Some(error),
        }
    }
}
