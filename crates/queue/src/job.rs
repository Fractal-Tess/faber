use faber_core::{Task, TaskResult};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use uuid::Uuid;

/// Unique identifier for a job
pub type JobId = String;

/// Execution job containing tasks to be processed
#[derive(Debug)]
pub struct ExecutionJob {
    pub id: JobId,
    pub tasks: Vec<Task>,
    pub created_at: Instant,
    pub status: JobStatus,
    pub result_sender: Option<oneshot::Sender<Vec<TaskResult>>>,
}

impl Clone for ExecutionJob {
    fn clone(&self) -> Self {
        // Clone everything except result_sender, which can't be cloned
        Self {
            id: self.id.clone(),
            tasks: self.tasks.clone(),
            created_at: self.created_at,
            status: self.status.clone(),
            result_sender: None, // Can't clone oneshot::Sender
        }
    }
}

impl ExecutionJob {
    pub fn new(tasks: Vec<Task>) -> (Self, oneshot::Receiver<Vec<TaskResult>>) {
        let (sender, receiver) = oneshot::channel();
        let job = Self {
            id: Uuid::new_v4().to_string(),
            tasks,
            created_at: Instant::now(),
            status: JobStatus::Queued,
            result_sender: Some(sender),
        };
        (job, receiver)
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    pub fn set_status(&mut self, status: JobStatus) {
        self.status = status;
    }

    pub fn complete_with_results(mut self, results: Vec<TaskResult>) {
        self.status = JobStatus::Completed;
        if let Some(sender) = self.result_sender.take() {
            let _ = sender.send(results);
        }
    }

    pub fn fail_with_error(mut self, error: String) {
        self.status = JobStatus::Failed;
        if let Some(sender) = self.result_sender.take() {
            // Send empty results with error info encoded in the first result
            let error_result = TaskResult {
                status: faber_core::TaskStatus::Failure,
                error: Some(error),
                exit_code: None,
                stdout: None,
                stderr: None,
                resource_usage: faber_core::ResourceUsage::new(),
                resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
            };
            let _ = sender.send(vec![error_result]);
        }
    }
}

/// Status of a job in the queue
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JobStatus {
    /// Job is waiting in the queue
    Queued,
    /// Job is currently being processed by a worker
    Running,
    /// Job completed successfully
    Completed,
    /// Job failed during execution
    Failed,
    /// Job was cancelled (timeout, queue shutdown, etc.)
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Queued => write!(f, "Queued"),
            JobStatus::Running => write!(f, "Running"),
            JobStatus::Completed => write!(f, "Completed"),
            JobStatus::Failed => write!(f, "Failed"),
            JobStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}
