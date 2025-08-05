use crate::error::{QueueError, QueueResult};
use crate::job::{ExecutionJob, JobStatus};
use faber_config::GlobalConfig;
use faber_core::TaskResult;
use faber_executor::Executor;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Worker that processes jobs from the queue
pub struct Worker {
    id: usize,
    config: Arc<GlobalConfig>,
    job_receiver: mpsc::UnboundedReceiver<ExecutionJob>,
}
impl Worker {
    pub fn new(
        id: usize,
        config: Arc<GlobalConfig>,
        job_receiver: mpsc::UnboundedReceiver<ExecutionJob>,
    ) -> Self {
        Self {
            id,
            config,
            job_receiver,
        }
    }

    /// Start the worker loop to process jobs
    pub async fn start(mut self) {
        info!("Worker {} starting", self.id);

        while let Some(mut job) = self.job_receiver.recv().await {
            let job_id = job.id.clone(); // Clone the ID before moving the job
            info!("Worker {} picked up job {}", self.id, job_id);
            job.set_status(JobStatus::Running);

            match self.process_job(&job).await {
                Ok(results) => {
                    info!("Worker {} completed job {} successfully", self.id, job_id);
                    if let Err(e) = job.complete_with_results(results).await {
                        error!(
                            "Worker {} failed to send results for job {}: {}",
                            self.id, job_id, e
                        );
                    }
                }
                Err(e) => {
                    error!("Worker {} failed to process job {}: {}", self.id, job_id, e);
                    if let Err(send_err) = job.fail_with_error(e.to_string()).await {
                        error!(
                            "Worker {} failed to send error for job {}: {}",
                            self.id, job_id, send_err
                        );
                    }
                }
            }
        }

        info!("Worker {} shutting down", self.id);
    }

    /// Process a single job by executing its tasks
    async fn process_job(&self, job: &ExecutionJob) -> QueueResult<Vec<TaskResult>> {
        let executor = Executor::new((*self.config).clone());

        // Set up timeout for job execution
        let execution_timeout =
            Duration::from_secs(self.config.queue.max_job_execution_time_seconds);

        debug!(
            "Worker {} executing {} tasks for job {}",
            self.id,
            job.tasks.len(),
            job.id
        );

        // Execute tasks with timeout
        match timeout(execution_timeout, executor.execute_tasks(&job.tasks)).await {
            Ok(Ok(results)) => Ok(results),
            Ok(Err(e)) => {
                let error_msg = format!("Task execution failed: {e}");
                Err(QueueError::ExecutionFailed { message: error_msg })
            }
            Err(_) => {
                warn!(
                    "Job {} timed out after {} seconds",
                    job.id, self.config.queue.max_job_execution_time_seconds
                );
                Err(QueueError::JobTimeout {
                    job_id: job.id.clone(),
                    timeout_seconds: self.config.queue.max_job_execution_time_seconds,
                })
            }
        }
    }
}
