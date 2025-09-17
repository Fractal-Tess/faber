use std::{
    io::{PipeWriter, Write},
    os::fd::IntoRawFd,
    process::{Command, exit},
};

use nix::{
    sys::wait::waitpid,
    unistd::{ForkResult, fork},
};

use crate::{
    ExecutionStep, ExecutionStepResult, Task, TaskGroup, TaskResult, TaskResultStats,
    container::Container,
    prelude::*,
    result::{RuntimeResult, TaskGroupResult},
    utils::{close_fd, mk_pipe},
};

pub struct Runtime {
    task_group: TaskGroup,
    container: Container,
}

impl Runtime {
    pub fn new(task_group: TaskGroup) -> Self {
        let container = Container::default();

        Self {
            task_group,
            container,
        }
    }

    pub fn execute(&self) -> Result<RuntimeResult> {
        let (reader, mut writer) = mk_pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                close_fd(reader.into_raw_fd())?;

                let runtime_result = self.execution_child();
                let _ = serde_json::to_writer(writer, &runtime_result);
                exit(0);
            }
            Ok(ForkResult::Parent { child }) => {
                close_fd(writer.into_raw_fd())?;
                waitpid(child, None);

                let runtime_result: RuntimeResult = match serde_json::from_reader(reader) {
                    Ok(result) => result,
                    Err(e) => {
                        println!("Failed to parse results from child process: {}", e);
                        return Err(FaberError::ParseResult {
                            e,
                            details: "Failed to parse results from child process".to_string(),
                        });
                    }
                };

                Ok(runtime_result)
            }
            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    fn execution_child(&self) -> RuntimeResult {
        // Handle container setup separately from task execution
        if let Err(e) = self.container.setup() {
            return RuntimeResult::ContainerSetupFailed {
                error: format!("Container setup failed: {}", e),
            };
        }

        let mut results = Vec::with_capacity(self.task_group.len());

        for step in &self.task_group {
            let result = match step {
                ExecutionStep::Single(task) => Self::execute_single(task.clone()),
                ExecutionStep::Parallel(tasks) => Self::execute_parallel(tasks.clone()),
            };
            results.push(result);
        }

        let _ = self.container.cleanup();
        RuntimeResult::Success(results)
    }

    fn execute_single(task: Task) -> ExecutionStepResult {
        match Self::execute_task(task) {
            Ok(task_result) => ExecutionStepResult::Single(task_result),
            Err(e) => ExecutionStepResult::Single(TaskResult::Failed {
                error: format!("Task execution failed: {}", e),
                stats: TaskResultStats::default(),
            }),
        }
    }

    fn execute_parallel(tasks: Vec<Task>) -> ExecutionStepResult {
        let mut handles = Vec::with_capacity(tasks.len());

        for task in tasks {
            let handle = std::thread::spawn(move || match Self::execute_task(task) {
                Ok(task_result) => task_result,
                Err(e) => TaskResult::Failed {
                    error: format!("Task execution failed: {}", e),
                    stats: TaskResultStats::default(),
                },
            });
            handles.push(handle);
        }

        // Wait for all threads to complete and collect results
        let task_results = Self::collect_parallel_results(handles);
        ExecutionStepResult::Parallel(task_results)
    }

    fn collect_parallel_results(
        handles: Vec<std::thread::JoinHandle<TaskResult>>,
    ) -> Vec<TaskResult> {
        let mut task_results = Vec::with_capacity(handles.len());

        for handle in handles {
            let result = match handle.join() {
                Ok(task_result) => task_result,
                Err(_) => TaskResult::Failed {
                    error: "Thread panicked during task execution".to_string(),
                    stats: TaskResultStats::default(),
                },
            };
            task_results.push(result);
        }

        task_results
    }

    fn execute_task(task: Task) -> Result<TaskResult> {
        let mut cmd = Command::new(task.cmd);

        for (key, value) in task.env.unwrap_or_default() {
            cmd.env(key, value);
        }

        let has_path = cmd.get_envs().any(|(key, _)| key == "PATH");
        if !has_path {
            cmd.env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            );
        }

        if let Some(args) = task.args {
            cmd.args(args);
        }

        let output = cmd.output().map_err(|e| FaberError::ExecuteTask {
            e,
            details: "Failed to execute task".to_string(),
        })?;
        let stdout = String::from_utf8(output.stdout).map_err(|e| FaberError::GetStdout {
            e,
            details: "Failed to convert stdout to string".to_string(),
        })?;
        let stderr = String::from_utf8(output.stderr).map_err(|e| FaberError::GetStderr {
            e,
            details: "Failed to convert stderr to string".to_string(),
        })?;

        let exit_code = output
            .status
            .code()
            .ok_or_else(|| FaberError::GetExitCode {
                e: std::io::Error::other("Failed to get exit code"),
                details: "Failed to get exit code".to_string(),
            })?;

        Ok(TaskResult::Completed {
            stdout,
            stderr,
            exit_code,
            stats: TaskResultStats::default(),
        })
    }
}
