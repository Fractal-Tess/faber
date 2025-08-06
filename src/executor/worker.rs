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

        // Initialize namespaces - temporarily disabled to fix EINVAL issue
        // TODO: Implement proper namespace isolation using fork/exec
        debug!(
            "Worker {} skipping namespace initialization for now",
            self.id
        );
        /*
        let mut namespaces =
            ContainerNamespaces::new(self.config.container.security.namespaces.clone());
        if let Err(e) = namespaces.initialize().await {
            error!("Worker {} failed to initialize namespaces: {}", self.id, e);
            return Err(Box::new(e));
        }
        self.namespaces = Some(namespaces);
        debug!("Worker {} namespaces initialized successfully", self.id);
        */
        self.namespaces = None;

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
        use nix::sys::wait::waitpid;
        use nix::unistd::{ForkResult, execvp, fork};
        use std::ffi::CString;
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::path::PathBuf;

        // Get container reference
        let container = self.container.as_ref().ok_or("Container not initialized")?;
        let container_root = container.container_root();

        debug!("Worker {} executing command in container", self.id);
        debug!("Worker {} container root: {:?}", self.id, container_root);

        // Create pipes for output capture using raw file descriptors
        let (stdout_read_fd, stdout_write_fd) = unsafe {
            let mut fds = [0; 2];
            if nix::libc::pipe(fds.as_mut_ptr()) != 0 {
                return Err("Failed to create stdout pipe".into());
            }
            (fds[0], fds[1])
        };

        let (stderr_read_fd, stderr_write_fd) = unsafe {
            let mut fds = [0; 2];
            if nix::libc::pipe(fds.as_mut_ptr()) != 0 {
                return Err("Failed to create stderr pipe".into());
            }
            (fds[0], fds[1])
        };

        // Fork the process
        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                // This is the child process

                // Change root to container filesystem
                nix::unistd::chroot(container_root).ok();
                nix::unistd::chdir("/").ok();

                // Close the read ends of the pipes in the child
                unsafe {
                    nix::libc::close(stdout_read_fd);
                    nix::libc::close(stderr_read_fd);
                }

                // Redirect stdout and stderr to our pipes
                unsafe {
                    nix::libc::dup2(stdout_write_fd, 1);
                    nix::libc::dup2(stderr_write_fd, 2);
                }

                // Close the write ends after redirection
                unsafe {
                    nix::libc::close(stdout_write_fd);
                    nix::libc::close(stderr_write_fd);
                }

                // Prepare command and arguments for exec
                let cmd_cstr = CString::new(task.cmd.as_str()).unwrap();

                // Build the full argument list: [cmd, ...args]
                let mut all_args = vec![task.cmd.clone()];
                if let Some(args) = &task.args {
                    all_args.extend(args.clone());
                }

                let args_cstr: Vec<CString> = all_args
                    .iter()
                    .map(|arg| CString::new(arg.as_str()).unwrap())
                    .collect();

                // Execute the command with the full argument list
                let _ = execvp(&cmd_cstr, &args_cstr);

                // If we get here, exec failed
                std::process::exit(1);
            }
            Ok(ForkResult::Parent { child }) => {
                // This is the parent process

                // Close the write ends of the pipes in the parent
                unsafe {
                    nix::libc::close(stdout_write_fd);
                    nix::libc::close(stderr_write_fd);
                }

                // Read from the pipes
                let mut stdout_buf = Vec::new();
                let mut stderr_buf = Vec::new();

                // Read stdout
                let mut stdout_file = unsafe { std::fs::File::from_raw_fd(stdout_read_fd) };
                std::io::Read::read_to_end(&mut stdout_file, &mut stdout_buf).ok();
                // Drop the file to close the fd
                drop(stdout_file);

                // Read stderr
                let mut stderr_file = unsafe { std::fs::File::from_raw_fd(stderr_read_fd) };
                std::io::Read::read_to_end(&mut stderr_file, &mut stderr_buf).ok();
                // Drop the file to close the fd
                drop(stderr_file);

                let stdout_str = String::from_utf8_lossy(&stdout_buf).to_string();
                let stderr_str = String::from_utf8_lossy(&stderr_buf).to_string();

                debug!("Worker {} read stdout: '{}'", self.id, stdout_str);
                debug!("Worker {} read stderr: '{}'", self.id, stderr_str);

                // Wait for child process to complete
                let wait_result = waitpid(Some(child), None);

                let exit_code = match wait_result {
                    Ok(nix::sys::wait::WaitStatus::Exited(_, code)) => code,
                    Ok(nix::sys::wait::WaitStatus::Signaled(_, signal, _)) => signal as i32,
                    _ => -1,
                };

                Ok((stdout_str, stderr_str, exit_code))
            }
            Err(e) => Err(format!("Failed to fork process: {e}").into()),
        }
    }
}
