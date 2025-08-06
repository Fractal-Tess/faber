use crate::config::FaberConfig;
use crate::container::{Container, ContainerNamespaces};
use crate::executor::task::{Task, TaskResult};

use std::process::Command;
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
    container: Option<Container>,
    namespaces: Option<ContainerNamespaces>,
    receiver: Receiver<WorkerMessage>,
}

impl Worker {
    pub fn new(id: u16, config: Arc<FaberConfig>, receiver: Receiver<WorkerMessage>) -> Self {
        Self {
            id,
            state: WorkerState::Initializing,
            config,
            container: None,
            namespaces: None,
            receiver,
        }
    }

    /// Start the worker lifecycle
    pub async fn run(mut self) {
        debug!("Worker {} starting up", self.id);

        // Initialize the worker
        if let Err(e) = self.initialize().await {
            error!("Worker {} failed to initialize: {}", self.id, e);
            return;
        }

        debug!("Worker {} ready and waiting for tasks", self.id);

        // Main worker loop
        while let Some(message) = self.receiver.recv().await {
            match message {
                WorkerMessage::ExecuteBatch {
                    tasks,
                    response_sender,
                } => {
                    self.state = WorkerState::Executing;
                    let task_count = tasks.len();
                    debug!(
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

                    debug!(
                        "Worker {} completed batch execution of {} tasks",
                        self.id, task_count
                    );

                    // Send results back
                    if let Err(e) = response_sender.send(results) {
                        error!("Worker {} failed to send results: {:?}", self.id, e);
                    }

                    self.state = WorkerState::Ready;
                }
                WorkerMessage::Shutdown => {
                    info!("Worker {} received shutdown message", self.id);
                    self.state = WorkerState::CleaningUp;
                    break;
                }
            }
        }

        debug!("Worker {} shutting down", self.id);
    }

    /// Initialize the worker
    async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Worker {} beginning initialization", self.id);
        self.state = WorkerState::Initializing;

        if let Some(mut container) = self.container.take() {
            container.cleanup().await?;
        }

        debug!("Worker {} creating container", self.id);
        let mut container = Container::new(self.config.clone());

        if let Err(e) = container.initialize().await {
            error!("Worker {} failed to initialize container: {}", self.id, e);
            return Err(Box::new(e));
        }

        self.container = Some(container);
        debug!("Worker {} container initialized successfully", self.id);

        // Initialize namespaces
        debug!("Worker {} initializing namespaces", self.id);
        let mut namespaces =
            ContainerNamespaces::new(self.config.container.security.namespaces.clone());
        if let Err(e) = namespaces.initialize().await {
            error!("Worker {} failed to initialize namespaces: {}", self.id, e);
            return Err(Box::new(e));
        }
        self.namespaces = Some(namespaces);
        debug!("Worker {} namespaces initialized successfully", self.id);

        self.state = WorkerState::Ready;
        info!("Worker {} initialization completed successfully", self.id);
        Ok(())
    }

    /// Execute a task
    async fn execute_task(&mut self, task: Task) -> TaskResult {
        let start_time = Instant::now();

        info!("Worker {} executing command: '{}'", self.id, task.cmd);
        if let Some(args) = &task.args {
            debug!("Worker {} command arguments: {:?}", self.id, args);
        }

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

    /// Execute the actual command in the container
    async fn execute_command(
        &self,
        task: &Task,
    ) -> Result<(String, String, i32), Box<dyn std::error::Error + Send + Sync>> {
        // Get container reference
        let container = self.container.as_ref().ok_or("Container not initialized")?;
        let namespaces = self
            .namespaces
            .as_ref()
            .ok_or("Namespaces not initialized")?;

        debug!("Worker {} executing command in container", self.id);

        // Prepare command arguments
        let empty_args = vec![];
        let args = task.args.as_ref().unwrap_or(&empty_args);
        let args_strings: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        // Set up namespaces
        namespaces
            .setup_environment()
            .map_err(|e| format!("Failed to set up namespaces: {e}"))?;

        // Execute the command directly in the current process
        let output = Command::new(&task.cmd)
            .args(&args_strings)
            .current_dir(&self.config.container.work_dir)
            .output()
            .map_err(|e| format!("Failed to execute command: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }
}
