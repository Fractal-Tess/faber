use crate::config::FaberConfig;
use crate::worker::instance::{Worker, WorkerMessage, WorkerState};
use crate::worker::task::{Task, TaskResult};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Worker pool that manages multiple workers
pub struct WorkerPool {
    config: Arc<FaberConfig>,
    workers: Vec<Sender<WorkerMessage>>,
    worker_handles: Vec<JoinHandle<()>>,
    next_worker: usize,
}

impl WorkerPool {
    /// Create a new worker pool
    pub async fn new(
        config: Arc<FaberConfig>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let worker_count = config.queue.worker_count;
        info!("Creating worker pool with {} workers", worker_count);

        let mut workers = Vec::with_capacity(worker_count as usize);
        let mut worker_handles = Vec::with_capacity(worker_count as usize);

        for worker_id in 0..worker_count {
            debug!("Creating worker {}", worker_id);
            let (sender, receiver) = mpsc::channel(100); // Buffer size for each worker
            let worker = Worker::new(worker_id, Arc::clone(&config), receiver);

            // Spawn worker task
            let handle = tokio::spawn(async move {
                worker.run().await;
            });

            workers.push(sender);
            worker_handles.push(handle);
            debug!("Worker {} created successfully", worker_id);
        }

        info!(
            "Worker pool created successfully with {} workers",
            worker_count
        );

        Ok(Self {
            config,
            workers,
            worker_handles,
            next_worker: 0,
        })
    }

    /// Execute a task using round-robin worker selection
    async fn execute_task(
        &mut self,
        task: Task,
    ) -> Result<TaskResult, Box<dyn std::error::Error + Send + Sync>> {
        if self.workers.is_empty() {
            return Err("No workers available".into());
        }

        // Round-robin worker selection
        let worker_id = self.next_worker;
        self.next_worker = (self.next_worker + 1) % self.workers.len();

        debug!("Assigning task to worker {} (round-robin)", worker_id);
        let worker_sender = &self.workers[worker_id];

        // Create response channel
        let (response_sender, response_receiver) = oneshot::channel();

        // Send task to worker
        let message = WorkerMessage::Execute {
            task,
            response_sender,
        };

        debug!("Sending task to worker {}", worker_id);
        worker_sender
            .send(message)
            .await
            .map_err(|e| format!("Failed to send task to worker {worker_id}: {e}"))?;

        // Wait for response
        debug!("Waiting for result from worker {}", worker_id);
        let result = response_receiver
            .await
            .map_err(|e| format!("Failed to receive result from worker {worker_id}: {e}"))?;

        info!("Task completed by worker {}: {:?}", worker_id, result);
        Ok(result)
    }

    pub async fn execute_tasks(
        &mut self,
        tasks: Vec<Task>,
    ) -> Result<Vec<TaskResult>, Box<dyn std::error::Error + Send + Sync>> {
        let task_count = tasks.len();
        info!("Starting execution of {} tasks", task_count);

        let mut results = Vec::with_capacity(task_count);

        // Execute tasks sequentially to avoid borrowing issues
        for (index, task) in tasks.into_iter().enumerate() {
            info!("Executing task {}/{}: {:?}", index + 1, task_count, task);
            match self.execute_task(task).await {
                Ok(result) => {
                    info!("Task {}/{} completed successfully", index + 1, task_count);
                    results.push(result);
                }
                Err(e) => {
                    error!("Task {}/{} execution failed: {}", index + 1, task_count, e);
                    // Create a failure result
                    results.push(TaskResult::failure(
                        e.to_string(),
                        std::time::Duration::ZERO,
                    ));
                }
            }
        }

        info!("Completed execution of all {} tasks", task_count);
        Ok(results)
    }

    /// Get the number of workers in the pool
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    /// Shutdown all workers gracefully
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down worker pool...");

        // Send shutdown signal to all workers
        for (worker_id, sender) in self.workers.iter().enumerate() {
            if let Err(e) = sender.send(WorkerMessage::Shutdown).await {
                warn!(
                    "Failed to send shutdown signal to worker {}: {}",
                    worker_id, e
                );
            }
        }

        // Wait for all workers to finish
        for (worker_id, handle) in self.worker_handles.drain(..).enumerate() {
            match handle.await {
                Ok(_) => debug!("Worker {} shut down successfully", worker_id),
                Err(e) => error!("Worker {} failed to shut down: {}", worker_id, e),
            }
        }

        info!("Worker pool shut down successfully");
        Ok(())
    }
}
