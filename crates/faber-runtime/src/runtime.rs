use std::{
    io::{PipeWriter, Write},
    os::fd::IntoRawFd,
    process::exit,
};

use nix::{
    sys::wait::waitpid,
    unistd::{ForkResult, fork},
};

use crate::{
    ExecutionStep, ExecutionStepResult, TaskGroup, TaskResult, TaskResultStats,
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

        let task_group_result: TaskGroupResult =
            vec![ExecutionStepResult::Single(TaskResult::Completed {
                stdout: "Hello, world!".to_string(),
                stderr: "".to_string(),
                exit_code: 0,
                stats: TaskResultStats::default(),
            })];

        // Cleanup container (ignore errors for now)
        let _ = self.container.cleanup();

        RuntimeResult::Success(task_group_result)
    }
}
