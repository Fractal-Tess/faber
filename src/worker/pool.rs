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

        let mut workers = Vec::with_capacity(worker_count.into());
        let mut worker_handles = Vec::with_capacity(worker_count.into());

        for worker_id in 1..worker_count + 1 {
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

        Ok(Self {
            config,
            workers,
            worker_handles,
            next_worker: 0,
        })
    }

    pub async fn execute_tasks(
        &mut self,
        tasks: Vec<Task>,
    ) -> Result<Vec<TaskResult>, Box<dyn std::error::Error + Send + Sync>> {
        let task_count = tasks.len();
        info!(
            "Starting batch execution of {} tasks with single worker",
            task_count
        );

        if self.workers.is_empty() {
            return Err("No workers available".into());
        }

        // Select a single worker for all tasks
        let worker_id = self.next_worker;
        self.next_worker = (self.next_worker + 1) % self.workers.len();

        info!(
            "Assigning batch of {} tasks to worker {}",
            task_count, worker_id
        );
        let worker_sender = &self.workers[worker_id];

        // Create response channel for the entire batch
        let (response_sender, response_receiver) = oneshot::channel();

        // Send batch to the selected worker
        let message = WorkerMessage::ExecuteBatch {
            tasks,
            response_sender,
        };

        debug!(
            "Sending batch of {} tasks to worker {}",
            task_count, worker_id
        );
        worker_sender
            .send(message)
            .await
            .map_err(|e| format!("Failed to send batch to worker {worker_id}: {e}"))?;

        // Wait for all results
        debug!("Waiting for batch results from worker {}", worker_id);
        match response_receiver.await {
            Ok(results) => {
                info!("Batch of {task_count} tasks completed successfully by worker {worker_id}");
                Ok(results)
            }
            Err(e) => {
                error!("Failed to receive batch results from worker {worker_id}: {e}");
                Err(format!("Failed to receive batch results: {e}").into())
            }
        }
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
