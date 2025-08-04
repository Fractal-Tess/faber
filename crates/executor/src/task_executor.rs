use faber_config::Config;
use faber_core::{Result, Task, TaskResult, TaskStatus};
use faber_sandbox::ContainerSandbox;
use tracing::{error, info, warn};

pub struct TaskExecutor {
    pub config: Config,
}

impl TaskExecutor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Execute all tasks in a single shared container
    pub async fn execute_tasks(&self, tasks: &[Task]) -> Result<Vec<TaskResult>> {
        info!("TaskExecutor starting execution of {} tasks", tasks.len());

        // Create a single container for all tasks
        let mut container = self.create_container_sandbox()?;
        let mut results: Vec<TaskResult> = Vec::with_capacity(tasks.len());

        // Execute each task sequentially in the same container
        for (task_index, task) in tasks.iter().enumerate() {
            // Check if we should skip remaining tasks due to any previous failure
            let should_skip = results.iter().any(|result| {
                result.status != TaskStatus::Success && result.status != TaskStatus::NotExecuted
            });

            if should_skip {
                warn!("Skipping task {} due to previous failure", task_index);
                let skipped_result = TaskResult {
                    status: TaskStatus::NotExecuted,
                    error: None,
                    exit_code: None,
                    stdout: None,
                    stderr: None,
                    resource_usage: faber_core::ResourceUsage::new(),
                    resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
                };
                results.push(skipped_result);
                continue;
            }

            // Execute the task in the shared container
            match self
                .execute_single_task(&mut container, task, task_index)
                .await
            {
                Ok(result) => {
                    info!("Task {} completed successfully", task_index);
                    results.push(result);
                }
                Err(error) => {
                    let error_context = format!(
                        "Task {} failed: Command '{}' with args {:?} failed with error: {}",
                        task_index, task.command, task.args, error
                    );
                    error!("{}", error_context);
                    let failed_result = TaskResult {
                        status: TaskStatus::Failure,
                        error: Some(error_context),
                        exit_code: None,
                        stdout: None,
                        stderr: None,
                        resource_usage: faber_core::ResourceUsage::new(),
                        resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
                    };
                    results.push(failed_result);
                }
            }
        }

        // Clean up the shared container after all tasks are done
        if let Err(e) = container.cleanup() {
            error!("Failed to cleanup shared container: {}", e);
        }

        info!("Completed execution of all {} tasks", tasks.len());
        Ok(results)
    }

    /// Execute a single task in the provided container
    async fn execute_single_task(
        &self,
        container: &mut ContainerSandbox,
        task: &Task,
        task_index: usize,
    ) -> Result<TaskResult> {
        info!("Executing task {}: {}", task_index, task.command);

        // Copy files into container if provided
        if let Some(files) = &task.files {
            if !files.is_empty() {
                container.copy_files_in(files).map_err(|e| {
                    faber_core::FaberError::Execution(format!(
                        "Failed to copy {} files into container for task {}: {}",
                        files.len(),
                        task_index,
                        e
                    ))
                })?;
            }
        }

        // Prepare environment variables
        let env = task.env.clone().unwrap_or_default();

        // Execute the command in the shared container
        let result = container
            .execute_command(&task.command, &task.args.clone().unwrap_or_default(), &env)
            .map_err(|e| {
                faber_core::FaberError::Execution(format!(
                    "Task {} execution failed: {}",
                    task_index, e
                ))
            })?;

        Ok(result)
    }

    /// Legacy method for backward compatibility - now uses shared container approach
    pub async fn execute_task(&self, task: &Task) -> Result<TaskResult> {
        let results = self.execute_tasks(&[task.clone()]).await?;
        Ok(results.into_iter().next().unwrap())
    }

    fn create_container_sandbox(&self) -> Result<ContainerSandbox> {
        // Create container sandbox directly from config using the new approach
        faber_sandbox::container::ContainerSandbox::from_config(&self.config).map_err(|e| {
            faber_core::FaberError::Execution(format!("Failed to create container: {}", e))
        })
    }
}
