use serde::Serialize;
use thiserror::Error;

use crate::executor::{TaskResult, error::ExecutionTaskError, task::TaskStatus};

#[derive(Error, Debug, Clone, Serialize)]
pub enum SandboxError {
    #[error("Container creation failed: {0}")]
    ContainerCreation(String),

    #[error("Mount operation failed: {0}")]
    MountFailed(String),

    #[error("Namespace setup failed: {0}")]
    NamespaceSetup(String),

    #[error("Resource limit enforcement failed: {0}")]
    ResourceLimitFailed(String),

    #[error("Container execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Sandbox cleanup failed: {0}")]
    CleanupFailed(String),

    #[error("Container is not active")]
    ContainerNotActive,

    #[error("File copy into container failed: {0}")]
    FileCopyFailed(String),

    #[error("Security setup failed: {0}")]
    SecuritySetup(String),
}

impl From<SandboxError> for TaskResult {
    fn from(error: SandboxError) -> Self {
        TaskResult {
            status: TaskStatus::Failure,
            error: Some(ExecutionTaskError::SandboxError(error.clone())),
            exit_code: None,
            stdout: None,
            stderr: None,
        }
    }
}
