use crate::error::QueueResult;
use crate::job::{ExecutionJob, JobStatus};
use faber_config::GlobalConfig;
use faber_container::{Container, TaskResult};
use faber_executor::Executor;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

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
        // Create a new container
        let container_config = faber_container::ContainerConfig::from_config(&self.config);
        let container = Container::new(container_config).map_err(|e| {
            crate::error::QueueError::ContainerError {
                message: format!("Failed to create container: {}", e),
            }
        })?;

        // Create new executor and pass in the container
        let mut executor = Executor::new(container, (*self.config).clone());

        // Tell executor to run all tasks in the job
        let results = executor.execute_tasks(&job.tasks).await.map_err(|e| {
            crate::error::QueueError::ExecutionFailed {
                message: format!("Task execution failed: {}", e),
            }
        })?;

        // Return the results
        Ok(results)
    }
}
