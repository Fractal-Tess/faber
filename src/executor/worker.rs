use crate::config::FaberConfig;
use crate::container::Container;
use crate::executor::task::{Task, TaskResult};
use nix::libc::{gid_t, uid_t};
use nix::sched::unshare;
use nix::unistd::{setgid, setuid};
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
    container: Option<Container>,
    receiver: Receiver<WorkerMessage>,
}

impl Worker {
    pub fn new(id: u16, config: Arc<FaberConfig>, receiver: Receiver<WorkerMessage>) -> Self {
        Self {
            id,
            state: WorkerState::Initializing,
            config,
            container: None,
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

        // Create and initialize container
        debug!("Worker {} creating container", self.id);
        let mut container = Container::new(self.config.clone());

        if let Err(e) = container.initialize().await {
            error!("Worker {} failed to initialize container: {}", self.id, e);
            return Err(Box::new(e));
        }

        self.container = Some(container);
        debug!("Worker {} container initialized successfully", self.id);

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
        // Get container reference
        let container = self.container.as_ref().ok_or("Container not initialized")?;

        debug!("Worker {} executing command in container", self.id);

        // Get container root path and namespace flags
        let root_path = container
            .get_root_path()
            .map_err(|e| format!("Failed to get container root path: {}", e))?;
        let clone_flags = container
            .get_clone_flags()
            .map_err(|e| format!("Failed to get namespace flags: {}", e))?;

        // Prepare command arguments
        let empty_args = vec![];
        let args = task.args.as_ref().unwrap_or(&empty_args);
        let args_strings: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        // Build command with namespace isolation
        let mut command = Command::new("unshare");

        // Add namespace flags based on clone_flags
        if clone_flags.contains(nix::sched::CloneFlags::CLONE_NEWNS) {
            command.arg("--mount");
        }
        if clone_flags.contains(nix::sched::CloneFlags::CLONE_NEWUTS) {
            command.arg("--uts");
        }
        if clone_flags.contains(nix::sched::CloneFlags::CLONE_NEWIPC) {
            command.arg("--ipc");
        }
        if clone_flags.contains(nix::sched::CloneFlags::CLONE_NEWNET) {
            command.arg("--net");
        }
        if clone_flags.contains(nix::sched::CloneFlags::CLONE_NEWPID) {
            command.arg("--pid");
        }
        if clone_flags.contains(nix::sched::CloneFlags::CLONE_NEWUSER) {
            command.arg("--user");
        }
        if clone_flags.contains(nix::sched::CloneFlags::CLONE_NEWCGROUP) {
            command.arg("--cgroup");
        }

        command.arg("--root").arg(root_path);
        command.arg("--wd").arg(&self.config.container.work_dir);
        command.arg(&task.cmd);
        command.args(&args_strings);

        // Execute command
        let output = command
            .output()
            .map_err(|e| format!("Failed to execute command: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        debug!("Worker {} command execution completed", self.id);

        Ok((stdout, stderr, exit_code))
    }

    /// Cleanup and reinitialize the worker
    async fn cleanup_and_reinitialize(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Worker {} beginning cleanup process", self.id);
        self.state = WorkerState::CleaningUp;

        // Cleanup container
        if let Some(mut container) = self.container.take() {
            debug!("Worker {} cleaning up container", self.id);
            if let Err(e) = container.cleanup().await {
                error!("Worker {} failed to cleanup container: {}", self.id, e);
            }
        }

        info!("Worker {} cleanup completed, reinitializing", self.id);

        // Reinitialize
        self.initialize().await?;

        Ok(())
    }
}
