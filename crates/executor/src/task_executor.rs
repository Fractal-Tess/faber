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
        // Create container configuration from loaded config
        let security_level = self.get_security_level();
        let resource_limits = self.get_resource_limits(security_level.clone());
        let namespace_settings = self.get_namespace_settings();

        let container_config = faber_sandbox::container::ContainerConfig::new(security_level)
            .with_resource_limits(resource_limits)
            .with_namespace_settings(namespace_settings)
            .with_user_ids(self.config.container.uid, self.config.container.gid);

        // Create container with configuration
        ContainerSandbox::new(container_config).map_err(|e| {
            faber_core::FaberError::Execution(format!("Failed to create container: {}", e))
        })
    }

    fn get_security_level(&self) -> faber_sandbox::container::SecurityLevel {
        match self.config.security.default_security_level.as_str() {
            "minimal" => faber_sandbox::container::SecurityLevel::Minimal,
            "maximum" => faber_sandbox::container::SecurityLevel::Maximum,
            _ => faber_sandbox::container::SecurityLevel::Standard,
        }
    }

    fn get_resource_limits(
        &self,
        _security_level: faber_sandbox::container::SecurityLevel,
    ) -> faber_sandbox::container::ResourceLimits {
        let limits = &self.config.resource_limits.default;

        faber_sandbox::container::ResourceLimits {
            memory_limit: limits.memory_limit,
            cpu_time_limit: limits.cpu_time_limit,
            wall_time_limit: limits.wall_time_limit,
            max_processes: limits.max_processes,
            max_fds: limits.max_fds,
            stack_limit: limits.stack_limit,
            data_segment_limit: limits.data_segment_limit,
            address_space_limit: limits.address_space_limit,
            cpu_rate_limit: limits.cpu_rate_limit,
            io_read_limit: limits.io_read_limit,
            io_write_limit: limits.io_write_limit,
        }
    }

    fn get_namespace_settings(&self) -> faber_sandbox::container::NamespaceSettings {
        let ns = &self.config.security.namespaces;

        faber_sandbox::container::NamespaceSettings {
            pid: ns.pid,
            mount: ns.mount,
            network: ns.network,
            ipc: ns.ipc,
            uts: ns.uts,
            user: ns.user,
            time: ns.time,
            cgroup: ns.cgroup,
        }
    }
}
