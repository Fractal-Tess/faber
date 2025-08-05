use faber_core::{FaberError, TaskResult, TaskStatus};
use serde::Serialize;
use thiserror::Error;

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

impl From<SandboxError> for FaberError {
    fn from(error: SandboxError) -> Self {
        FaberError::Sandbox(error.to_string())
    }
}

impl From<SandboxError> for TaskResult {
    fn from(error: SandboxError) -> Self {
        TaskResult {
            status: TaskStatus::Failure,
            error: Some(error.to_string()),
            exit_code: None,
            stdout: None,
            stderr: None,
            resource_usage: faber_core::ResourceUsage::new(),
            resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
        }
    }
}
