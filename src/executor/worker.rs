use crate::config::FaberConfig;
use crate::executor::task::{Task, TaskResult};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

/// Worker state
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerState {
    Initializing,
    Ready,
    Executing,
    CleaningUp,
}

/// Message sent to worker
#[derive(Debug)]
pub enum WorkerMessage {
    Execute {
        task: Task,
        response_sender: oneshot::Sender<TaskResult>,
    },
    ExecuteBatch {
        tasks: Vec<Task>,
        response_sender: oneshot::Sender<Vec<TaskResult>>,
    },
    Shutdown,
}

/// Individual worker that processes tasks
pub struct Worker {
    id: u16,
    state: WorkerState,
    config: Arc<FaberConfig>,
    receiver: Receiver<WorkerMessage>,
}

impl Worker {
    pub fn new(id: u16, config: Arc<FaberConfig>, receiver: Receiver<WorkerMessage>) -> Self {
        Self {
            id,
            state: WorkerState::Initializing,
            config,
            receiver,
        }
    }

    /// Start the worker lifecycle
    pub async fn run(mut self) {
        info!("Worker {} starting up", self.id);

        // Initialize the worker
        if let Err(e) = self.initialize().await {
            error!("Worker {} failed to initialize: {}", self.id, e);
            return;
        }

        info!("Worker {} ready and waiting for tasks", self.id);

        // Main worker loop
        while let Some(message) = self.receiver.recv().await {
            match message {
                WorkerMessage::Execute {
                    task,
                    response_sender,
                } => {
                    self.state = WorkerState::Executing;
                    info!("Worker {} starting task execution: {:?}", self.id, task);

                    let result = self.execute_task(task).await;

                    info!(
                        "Worker {} completed task execution with result: {:?}",
                        self.id, result
                    );

                    // Send result back to the API
                    if let Err(e) = response_sender.send(result) {
                        error!("Worker {} failed to send result: {:?}", self.id, e);
                    }

                    // Cleanup and reinitialize
                    if let Err(e) = self.cleanup_and_reinitialize().await {
                        error!("Worker {} failed to cleanup/reinitialize: {}", self.id, e);
                        break;
                    }

                    info!("Worker {} ready for next task", self.id);
                }
                WorkerMessage::ExecuteBatch {
                    tasks,
                    response_sender,
                } => {
                    self.state = WorkerState::Executing;
                    let task_count = tasks.len();
                    info!(
                        "Worker {} starting batch execution of {} tasks",
                        self.id, task_count
                    );

                    let mut results = Vec::with_capacity(task_count);

                    // Execute all tasks in the batch
                    for (index, task) in tasks.into_iter().enumerate() {
                        info!(
                            "Worker {} executing task {}/{}: {:?}",
                            self.id,
                            index + 1,
                            task_count,
                            task
                        );

                        let result = self.execute_task(task).await;
                        info!(
                            "Worker {} completed task {}/{} with result: {:?}",
                            self.id,
                            index + 1,
                            task_count,
                            result
                        );
                        results.push(result);
                    }

                    info!(
                        "Worker {} completed batch execution of {} tasks",
                        self.id,
                        results.len()
                    );

                    // Send all results back to the API
                    if let Err(e) = response_sender.send(results) {
                        error!("Worker {} failed to send batch results: {:?}", self.id, e);
                    }

                    // Cleanup and reinitialize after the entire batch
                    if let Err(e) = self.cleanup_and_reinitialize().await {
                        error!(
                            "Worker {} failed to cleanup/reinitialize after batch: {}",
                            self.id, e
                        );
                        break;
                    }

                    info!("Worker {} ready for next batch", self.id);
                }
                WorkerMessage::Shutdown => {
                    info!("Worker {} received shutdown signal", self.id);
                    break;
                }
            }
        }

        info!("Worker {} shutting down", self.id);
    }

    /// Initialize the worker
    async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Worker {} beginning initialization", self.id);
        self.state = WorkerState::Initializing;

        // TODO: Add worker-specific initialization tasks here
        // For example:
        // - Set up container environment
        // - Prepare file system
        // - Initialize security context
        // - Set up networking

        debug!("Worker {} setting up container environment", self.id);
        // Simulate container setup
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        debug!("Worker {} preparing file system", self.id);
        // Simulate filesystem preparation
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

        debug!("Worker {} initializing security context", self.id);
        // Simulate security initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;

        self.state = WorkerState::Ready;
        info!("Worker {} initialization completed successfully", self.id);
        Ok(())
    }

    /// Execute a task
    async fn execute_task(&self, task: Task) -> TaskResult {
        let start_time = Instant::now();

        info!("Worker {} executing command: '{}'", self.id, task.cmd);
        if let Some(args) = &task.args {
            debug!("Worker {} command arguments: {:?}", self.id, args);
        }

        // TODO: Implement actual task execution
        // This is a placeholder implementation
        match self.execute_command(&task).await {
            Ok((stdout, stderr, exit_code)) => {
                let duration = start_time.elapsed();
                info!(
                    "Worker {} command completed successfully in {:?} (exit code: {})",
                    self.id, duration, exit_code
                );
                debug!("Worker {} stdout: {}", self.id, stdout);
                if !stderr.is_empty() {
                    debug!("Worker {} stderr: {}", self.id, stderr);
                }
                TaskResult::success(stdout, stderr, exit_code, duration)
            }
            Err(e) => {
                let duration = start_time.elapsed();
                error!(
                    "Worker {} command failed after {:?}: {}",
                    self.id, duration, e
                );
                TaskResult::failure(e.to_string(), duration)
            }
        }
    }

    /// Execute the actual command
    async fn execute_command(
        &self,
        task: &Task,
    ) -> Result<(String, String, i32), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: Implement actual command execution

        debug!(
            "Worker {} setting up container environment for command",
            self.id
        );
        // Simulate container setup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if let Some(env_vars) = &task.env {
            debug!(
                "Worker {} applying {} environment variables",
                self.id,
                env_vars.len()
            );
        }

        if let Some(files) = &task.files {
            debug!("Worker {} creating {} files", self.id, files.len());
        }

        // Placeholder implementation
        let empty_args = vec![];
        let args = task.args.as_ref().unwrap_or(&empty_args);
        let cmd_with_args = format!("{} {}", task.cmd, args.join(" "));

        debug!("Worker {} executing command: {}", self.id, cmd_with_args);

        // Simulate execution
        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

        debug!("Worker {} command execution completed", self.id);

        // Simulate success
        Ok((
            format!("Command executed successfully: {}", cmd_with_args),
            "No errors".to_string(),
            0,
        ))
    }

    /// Cleanup and reinitialize the worker
    async fn cleanup_and_reinitialize(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Worker {} beginning cleanup process", self.id);
        self.state = WorkerState::CleaningUp;

        // TODO: Add cleanup tasks here
        // For example:
        // - Clean up container resources
        // - Remove temporary files
        // - Reset security context
        // - Clean up networking

        debug!("Worker {} cleaning up container resources", self.id);
        // Simulate container cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;

        debug!("Worker {} removing temporary files", self.id);
        // Simulate file cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(15)).await;

        debug!("Worker {} resetting security context", self.id);
        // Simulate security reset
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        info!("Worker {} cleanup completed, reinitializing", self.id);

        // Reinitialize
        self.initialize().await?;

        Ok(())
    }
}
