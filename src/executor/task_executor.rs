use super::error::ExecutionTaskError;
use super::task::{ResourceLimitViolations, ResourceUsage, Task, TaskResult, TaskStatus};
use crate::config::Config;
use crate::sandbox::ContainerSandbox;
use crate::sandbox::container::ContainerConfig;
use crate::sandbox::error::SandboxError;
use tracing::{error, info, warn};

pub struct TaskExecutor {
    tasks: Vec<Task>,
    container: ContainerSandbox,
}

impl TaskExecutor {
    pub fn new(tasks: Vec<Task>, config: &Config) -> Result<Self, ExecutionTaskError> {
        // Create container configuration from loaded config
        let security_level = config.get_security_level();
        let resource_limits = config.get_resource_limits(security_level);
        let namespace_settings = config.get_namespace_settings();

        let container_config = ContainerConfig::new(security_level)
            .with_resource_limits(resource_limits)
            .with_namespace_settings(namespace_settings)
            .with_user_ids(config.container.uid, config.container.gid);

        // Create container with configuration
        let container =
            ContainerSandbox::new(container_config).map_err(ExecutionTaskError::SandboxError)?;

        Ok(Self { tasks, container })
    }

    pub fn execute(mut self) -> Vec<TaskResult> {
        info!(
            "TaskExecutor starting execution of {} tasks in container {}",
            self.tasks.len(),
            self.container.container_id()
        );

        let mut results: Vec<TaskResult> = Vec::with_capacity(self.tasks.len());

        // Clone tasks to avoid borrowing issues
        let tasks = self.tasks.clone();

        // Execute each task sequentially
        for (task_index, task) in tasks.iter().enumerate() {
            // Check if we should skip remaining tasks due to any previous failure
            let should_skip = results.iter().any(|result| result.error.is_some());

            if should_skip {
                warn!("Skipping task {} due to previous failure", task_index);
                let skipped_result = TaskResult {
                    status: TaskStatus::NotExecuted,
                    error: None,
                    exit_code: None,
                    stdout: None,
                    stderr: None,
                    resource_usage: ResourceUsage::new(),
                    resource_limits_exceeded: ResourceLimitViolations::new(),
                };
                results.push(skipped_result);
                continue;
            }

            // Execute the task and handle the result properly
            match self.execute_single_task(task, task_index) {
                Ok(result) => {
                    info!("Task {} completed successfully", task_index);
                    results.push(result);
                }
                Err(sandbox_error) => {
                    let error_context = format!(
                        "Task {} failed: Command '{}' with args {:?} failed with sandbox error: {}",
                        task_index, task.command, task.args, sandbox_error
                    );
                    error!("{}", error_context);
                    let failed_result = TaskResult {
                        status: TaskStatus::Failure,
                        error: Some(ExecutionTaskError::SandboxError(sandbox_error)),
                        exit_code: None,
                        stdout: None,
                        stderr: None,
                        resource_usage: ResourceUsage::new(),
                        resource_limits_exceeded: ResourceLimitViolations::new(),
                    };
                    results.push(failed_result);
                }
            }
        }

        info!("Completed execution of all tasks");
        results
    }

    fn execute_single_task(
        &mut self,
        task: &Task,
        task_index: usize,
    ) -> Result<TaskResult, SandboxError> {
        // Step 1: Copy task files into container if provided
        if let Some(files) = &task.files {
            if !files.is_empty() {
                if let Err(e) = self.container.copy_files_in(files) {
                    let error_context = format!(
                        "Task {}: Failed to copy {} files into container. Error: {}",
                        task_index,
                        files.len(),
                        e
                    );
                    error!("{}", error_context);
                    return Err(SandboxError::FileCopyFailed(error_context));
                }
            }
        }

        // Step 2: Prepare environment variables
        let env = task.env.clone().unwrap_or_default();

        // Step 3: Execute the command in the secure container
        match self.container.execute_command(
            &task.command,
            &task.args.clone().unwrap_or_default(),
            &env,
        ) {
            Ok(result) => Ok(result),
            Err(sandbox_error) => {
                let error_context = format!(
                    "Task {}: Command '{}' failed. Args: {:?}. Environment variables: {:?}. Sandbox error: {}",
                    task_index, task.command, task.args, env, sandbox_error
                );
                error!("{}", error_context);
                Err(sandbox_error)
            }
        }
    }
}
