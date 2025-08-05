use faber_container::SandboxError;
use faber_core::FaberError;
use serde::Serialize;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug, Clone, Serialize)]
pub enum ExecutionTaskError {
    #[error("Memory limit exceeded. Max allowed memory: {0:?}")]
    MemoryLimitExceeded(u64),

    #[error("CPU time limit exceeded. Max allowed time: {0:?}")]
    CpuTimeLimitExceeded(Duration),

    #[error("Wall time limit exceeded. Max allowed time: {0:?}")]
    WallTimeLimitExceeded(Duration),

    #[error("Process limit exceeded. Max allowed processes: {0}")]
    ProcessLimitExceeded(u32),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Sandbox error: {0}")]
    SandboxError(SandboxError),

    #[error("Task validation failed: {0}")]
    ValidationError(String),
}

impl From<ExecutionTaskError> for FaberError {
    fn from(error: ExecutionTaskError) -> Self {
        FaberError::Execution(error.to_string())
    }
}
