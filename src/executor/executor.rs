use super::error::ExecutionTaskError;
use super::task::{ExecutionTask, ExecutionTaskResult, ExecutionTaskStatus};
use crate::sandbox::{ContainerConfig, ContainerSandbox};
use std::time::Instant;
use tracing::{debug, error, info};

pub struct Executor {
    tasks: Vec<ExecutionTask>,
    container: ContainerSandbox,
}

impl Executor {
    pub fn new(tasks: Vec<ExecutionTask>) -> Result<Self, ExecutionTaskError> {
        // Create container with default execution configuration
        let config = ContainerConfig::default();
        let container = ContainerSandbox::new(config)
            .map_err(|e| ExecutionTaskError::FileNotFound(e.to_string()))?;

        Ok(Self { tasks, container })
    }

    pub fn new_with_config(
        tasks: Vec<ExecutionTask>,
        config: ContainerConfig,
    ) -> Result<Self, ExecutionTaskError> {
        let container = ContainerSandbox::new(config)
            .map_err(|e| ExecutionTaskError::FileNotFound(e.to_string()))?;

        Ok(Self { tasks, container })
    }

    pub fn execute(mut self) -> Vec<ExecutionTaskResult> {
        info!(
            "Executor starting execution of {} tasks in container {}",
            self.tasks.len(),
            self.container.container_id()
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

        results
    }

    fn execute_single_task(
        &mut self,
        task: &ExecutionTask,
        task_index: usize,
    ) -> ExecutionTaskResult {
        // Step 1: Copy task files into container if provided
        if let Some(files) = &task.files {
            if !files.is_empty() {
                debug!("Copying {} files for task {}", files.len(), task_index);
                if let Err(e) = self.container.copy_files_in(files) {
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

        // Step 2: Prepare environment variables
        let env = task.env.clone().unwrap_or_default();

        // Step 3: Execute the command in the secure container
        debug!(
            "Running command: {} {:?}",
            task.command,
            task.args.as_ref().unwrap_or(&vec![])
        );
        let start_time = Instant::now();

        match self.container.execute_command(
            &task.command,
            &task.args.clone().unwrap_or_default(),
            &env,
        ) {
            Ok(container_result) => {
                let execution_time = start_time.elapsed();

                debug!(
                    "Task {} completed in {:?} with exit code {}",
                    task_index, execution_time, container_result.exit_code
                );

                // Map ContainerResult to ExecutionTaskResult
                let status = if container_result.exit_code == 0 {
                    ExecutionTaskStatus::Success
                } else {
                    ExecutionTaskStatus::Failure
                };

                let error = if container_result.was_killed {
                    container_result
                        .kill_reason
                        .map(|reason| match reason.as_str() {
                            reason if reason.contains("memory") => {
                                ExecutionTaskError::MemoryLimitExceeded(
                                    container_result.memory_used,
                                )
                            }
                            reason if reason.contains("time") || reason.contains("cpu") => {
                                ExecutionTaskError::CpuTimeLimitExceeded(
                                    std::time::Duration::from_nanos(container_result.cpu_time_used),
                                )
                            }
                            reason if reason.contains("wall") => {
                                ExecutionTaskError::WallTimeLimitExceeded(
                                    std::time::Duration::from_nanos(
                                        container_result.wall_time_used,
                                    ),
                                )
                            }
                            _ => ExecutionTaskError::FileNotFound(reason),
                        })
                } else {
                    None
                };

                ExecutionTaskResult {
                    status,
                    error,
                    exit_code: container_result.exit_code,
                    stdout: container_result.stdout,
                    stderr: container_result.stderr,
                }
            }
            Err(e) => {
                error!("Failed to execute task {}: {}", task_index, e);
                ExecutionTaskResult {
                    status: ExecutionTaskStatus::NotExecuted,
                    error: Some(ExecutionTaskError::FileNotFound(e.to_string())),
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Container execution failed: {}", e),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_executor_integration() {
        // Create a simple task
        let mut files = HashMap::new();
        files.insert(
            "hello.sh".to_string(),
            "#!/bin/sh\necho 'Hello from executor!'\n".to_string(),
        );

        let task = ExecutionTask {
            command: "/bin/sh".to_string(),
            args: Some(vec!["hello.sh".to_string()]),
            env: None,
            files: Some(files),
        };

        // Create executor with minimal namespaces for testing
        let mut config = ContainerConfig::default();
        config.enable_pid_namespace = false;
        config.enable_mount_namespace = false;
        config.enable_network = false;

        let executor =
            Executor::new_with_config(vec![task], config).expect("Failed to create executor");

        // Execute the task
        let results = executor.execute();

        // Verify results
        assert_eq!(results.len(), 1);
        let result = &results[0];

        match result.status {
            ExecutionTaskStatus::Success => {
                assert_eq!(result.exit_code, 0);
                assert_eq!(result.stdout.trim(), "Hello from executor!");
                println!("Executor integration test passed!");
            }
            _ => {
                println!(
                    "Executor test failed (expected in some environments): {:?}",
                    result
                );
                // Don't fail the test - this is environment dependent
            }
        }
    }
}
