use faber_config::GlobalConfig;
use faber_container::Container;
use faber_core::{Result, Task, TaskResult, TaskStatus};
use tracing::{debug, error, info, warn};

pub struct Executor {
    pub config: GlobalConfig,
}

impl Executor {
    pub fn new(config: GlobalConfig) -> Self {
        Self { config }
    }

    pub fn setup_container(&self) -> Result<Container> {
        debug!("🛠️  === Setting up container ===");

        Container::from_config(&self.config).map_err(|e| {
            faber_core::FaberError::Execution(format!("Failed to create container: {e}"))
        })
    }

    /// Execute all tasks in a single shared container
    pub async fn execute_tasks(&self, tasks: &[Task]) -> Result<Vec<TaskResult>> {
        info!("Executor starting execution of {} tasks", tasks.len());

        // Create a single container for all tasks
        let mut container = self.create_container_sandbox()?;
        let mut task_results: Vec<TaskResult> = Vec::with_capacity(tasks.len());
        let mut cleanup_error: Option<String> = None;

        // Execute each task sequentially in the same container
        for (task_index, task) in tasks.iter().enumerate() {
            // Check if we should skip remaining tasks due to any previous failure
            let should_skip = if self.config.executor.fail_fast {
                task_results.iter().any(|result| {
                    result.status != TaskStatus::Success && result.status != TaskStatus::NotExecuted
                })
            } else {
                false
            };

            if should_skip {
                warn!("Skipping task {task_index} due to previous failure (fail_fast=true)");
                let skipped_result = TaskResult {
                    status: TaskStatus::NotExecuted,
                    error: Some("Skipped due to previous task failure".to_string()),
                    exit_code: None,
                    stdout: None,
                    stderr: None,
                    resource_usage: faber_core::ResourceUsage::new(),
                    resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
                };
                task_results.push(skipped_result);
                continue;
            }

            // Execute the task in the shared container
            match self
                .execute_single_task(&mut container, task, task_index)
                .await
            {
                Ok(result) => {
                    info!("Task {} completed successfully", task_index);
                    task_results.push(result);
                }
                Err(error) => {
                    let error_context = format!(
                        "Task {task_index} failed: Command '{task:?}' failed with error: {error}"
                    );
                    error!("{error_context}");
                    let failed_result = TaskResult {
                        status: TaskStatus::Failure,
                        error: Some(error_context),
                        exit_code: None,
                        stdout: None,
                        stderr: None,
                        resource_usage: faber_core::ResourceUsage::new(),
                        resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
                    };
                    task_results.push(failed_result);
                }
            }
        }

        // Clean up the shared container after all tasks are done
        if let Err(e) = container.cleanup() {
            let cleanup_err = format!("Failed to cleanup shared container: {e}");
            error!("{cleanup_err}");
            cleanup_error = Some(cleanup_err);
        }

        // If cleanup failed and we have results, add the cleanup error to the last result
        if let Some(cleanup_err) = cleanup_error {
            if let Some(last_result) = task_results.last_mut() {
                // Append cleanup error to the last result's error message
                let existing_error = last_result.error.take().unwrap_or_default();
                last_result.error = Some(if existing_error.is_empty() {
                    cleanup_err
                } else {
                    format!("{existing_error}; {cleanup_err}")
                });
            } else {
                // No results, create an error result for cleanup failure
                task_results.push(TaskResult {
                    status: TaskStatus::Failure,
                    error: Some(cleanup_err),
                    exit_code: None,
                    stdout: None,
                    stderr: None,
                    resource_usage: faber_core::ResourceUsage::new(),
                    resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
                });
            }
        }

        info!("Completed execution of all {} tasks", tasks.len());
        Ok(task_results)
    }

    /// Execute a single task in the provided container
    async fn execute_single_task(
        &self,
        container: &mut Container,
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

        debug!("Environment variables: {env:?}");

        // Execute the command in the shared container
        let result = container
            .execute_command(&task.command, &task.args.clone().unwrap_or_default(), &env)
            .map_err(|e| {
                faber_core::FaberError::Execution(format!(
                    "Task {task_index} execution failed: {e}"
                ))
            })?;

        Ok(result)
    }

    /// Legacy method for backward compatibility - now uses shared container approach
    pub async fn execute_task(&self, task: &Task) -> Result<TaskResult> {
        let results = self.execute_tasks(&[task.clone()]).await?;
        Ok(results.into_iter().next().unwrap())
    }

    fn create_container_sandbox(&self) -> Result<Container> {
        // Create container sandbox directly from config using the new approach
        faber_container::container::Container::from_config(&self.config).map_err(|e| {
            faber_core::FaberError::Execution(format!("Failed to create container: {e}"))
        })
    }
}
