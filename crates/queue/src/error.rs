use thiserror::Error;

/// Queue-related errors
#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Queue is full: cannot accept more jobs")]
    QueueFull,

    #[error("Job timeout: job {job_id} timed out after {timeout_seconds} seconds")]
    JobTimeout {
        job_id: String,
        timeout_seconds: u64,
    },

    #[error("Job execution failed: {message}")]
    ExecutionFailed { message: String },

    #[error("Job not found: {job_id}")]
    JobNotFound { job_id: String },

    #[error("Worker error: {message}")]
    WorkerError { message: String },

    #[error("Queue manager error: {message}")]
    QueueManagerError { message: String },

    #[error("Job cancelled: {reason}")]
    JobCancelled { reason: String },
}

pub type QueueResult<T> = Result<T, QueueError>;
