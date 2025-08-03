use super::error::ExecutionTaskError;
use super::task::{ExecutionTask, ExecutionTaskResult, ExecutionTaskStatus};
use crate::sandbox::Sandbox;
use std::process::Command;
use std::time::Instant;
use tracing::{debug, error, info};

pub struct Executor {
    tasks: Vec<ExecutionTask>,
    sandbox: Sandbox,
}

impl Executor {
    pub fn new(tasks: Vec<ExecutionTask>, sandbox: Sandbox) -> Self {
        Self { tasks, sandbox }
    }

    pub fn execute(mut self) -> Vec<ExecutionTaskResult> {
        info!(
            "Executor starting execution of {} tasks in sandbox {}",
            self.tasks.len(),
            self.sandbox.sandbox_id()
        );

        let mut results = Vec::with_capacity(self.tasks.len());

        // Clone tasks to avoid borrowing issues
        let tasks = self.tasks.clone();

        // Execute each task sequentially
        for (task_index, task) in tasks.iter().enumerate() {
            debug!(
                "Executing task {}/{}: {}",
                task_index + 1,
                tasks.len(),
                task.command
            );

            let result = self.execute_single_task(task, task_index);
            results.push(result);
        }

        info!("Completed execution of all tasks");

        // Cleanup the sandbox before returning
        if let Err(e) = self.sandbox.cleanup() {
            error!("Failed to cleanup sandbox during execution: {}", e);
        }

        results
    }

    fn execute_single_task(
        &mut self,
        task: &ExecutionTask,
        task_index: usize,
    ) -> ExecutionTaskResult {
        // Step 1: Copy task files into sandbox if provided
        if let Some(files) = &task.files {
            if !files.is_empty() {
                debug!("Copying {} files for task {}", files.len(), task_index);
                if let Err(e) = self.sandbox.copy_files_in(files) {
                    error!("Failed to copy files for task {}: {}", task_index, e);
                    return ExecutionTaskResult {
                        status: ExecutionTaskStatus::NotExecuted,
                        error: Some(ExecutionTaskError::FileNotFound(e.to_string())),
                        exit_code: -1,
                        stdout: String::new(),
                        stderr: format!("Failed to copy files: {e}"),
                    };
                }
            }
        }

        // Step 2: Prepare the command
        let mut cmd = Command::new(&task.command);

        // Add arguments if provided
        if let Some(args) = &task.args {
            cmd.args(args);
        }

        // Apply sandbox limits and environment
        if let Err(e) = self.sandbox.apply_limits(&mut cmd) {
            error!(
                "Failed to apply sandbox limits for task {}: {}",
                task_index, e
            );
            return ExecutionTaskResult {
                status: ExecutionTaskStatus::NotExecuted,
                error: Some(ExecutionTaskError::FileNotFound(e.to_string())),
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Failed to apply sandbox limits: {e}"),
            };
        }

        // Add task-specific environment variables
        if let Some(env_vars) = &task.env {
            for (key, value) in env_vars {
                cmd.env(key, value);
            }
        }

        // Step 3: Execute the command
        debug!(
            "Running command: {} {:?}",
            task.command,
            task.args.as_ref().unwrap_or(&vec![])
        );
        let start_time = Instant::now();

        match cmd.output() {
            Ok(output) => {
                let execution_time = start_time.elapsed();
                let exit_code = output.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                debug!(
                    "Task {} completed in {:?} with exit code {}",
                    task_index, execution_time, exit_code
                );

                let status = if output.status.success() {
                    ExecutionTaskStatus::Success
                } else {
                    ExecutionTaskStatus::Failure
                };

                ExecutionTaskResult {
                    status,
                    error: None,
                    exit_code,
                    stdout,
                    stderr,
                }
            }
            Err(e) => {
                error!("Failed to execute task {}: {}", task_index, e);
                ExecutionTaskResult {
                    status: ExecutionTaskStatus::NotExecuted,
                    error: Some(ExecutionTaskError::FileNotFound(e.to_string())),
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Process execution failed: {}", e),
                }
            }
        }
    }
}
