use crate::config::FaberConfig;
use crate::executor::task::{Task, TaskResult};
use crate::executor::worker::{Worker, WorkerMessage};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Executor pool that manages multiple executor workers
pub struct ExecutorPool {
    config: Arc<FaberConfig>,
    executors: Vec<Sender<WorkerMessage>>,
    executor_handles: Vec<JoinHandle<()>>,
    next_executor: usize,
}

impl ExecutorPool {
    /// Create a new executor pool
    pub async fn new(
        config: Arc<FaberConfig>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let executor_count = config.executor.worker_count;
        info!("Creating executor pool with {} executors", executor_count);

        let mut executors = Vec::with_capacity(executor_count.into());
        let mut executor_handles = Vec::with_capacity(executor_count.into());

        for executor_id in 1..executor_count + 1 {
            debug!("Creating executor {}", executor_id);
            let (sender, receiver) = mpsc::channel(100); // Buffer size for each executor
            let worker = Worker::new(executor_id, Arc::clone(&config), receiver);

            // Spawn executor task
            let handle = tokio::spawn(async move {
                worker.run().await;
            });

            executors.push(sender);
            executor_handles.push(handle);
            debug!("Executor {} created successfully", executor_id);
        }

        Ok(Self {
            config,
            executors,
            executor_handles,
            next_executor: 0,
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

        if self.executors.is_empty() {
            return Err("No executors available".into());
        }

        // Select a single executor for all tasks
        let executor_id = self.next_executor;
        self.next_executor = (self.next_executor + 1) % self.executors.len();

        info!(
            "Assigning batch of {} tasks to executor {}",
            task_count, executor_id
        );
        let executor_sender = &self.executors[executor_id];

        // Create response channel for the entire batch
        let (response_sender, response_receiver) = oneshot::channel();

        // Send batch to the selected worker
        let message = WorkerMessage::ExecuteBatch {
            tasks,
            response_sender,
        };

        debug!(
            "Sending batch of {} tasks to executor {}",
            task_count, executor_id
        );
        executor_sender
            .send(message)
            .await
            .map_err(|e| format!("Failed to send batch to executor {executor_id}: {e}"))?;

        // Wait for all results
        debug!("Waiting for batch results from executor {}", executor_id);
        match response_receiver.await {
            Ok(results) => {
                info!(
                    "Batch of {task_count} tasks completed successfully by executor {executor_id}"
                );
                Ok(results)
            }
            Err(e) => {
                error!("Failed to receive batch results from executor {executor_id}: {e}");
                Err(format!("Failed to receive batch results: {e}").into())
            }
        }
    }

    /// Get the number of executors in the pool
    pub fn executor_count(&self) -> usize {
        self.executors.len()
    }

    /// Shutdown all executors gracefully
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down executor pool...");

        // Send shutdown signal to all executors
        for (executor_id, sender) in self.executors.iter().enumerate() {
            if let Err(e) = sender.send(WorkerMessage::Shutdown).await {
                warn!(
                    "Failed to send shutdown signal to executor {}: {}",
                    executor_id, e
                );
            }
        }

        // Wait for all executors to finish
        for (executor_id, handle) in self.executor_handles.drain(..).enumerate() {
            match handle.await {
                Ok(_) => debug!("Executor {} shut down successfully", executor_id),
                Err(e) => error!("Executor {} failed to shut down: {}", executor_id, e),
            }
        }

        info!("Executor pool shut down successfully");
        Ok(())
    }
}
