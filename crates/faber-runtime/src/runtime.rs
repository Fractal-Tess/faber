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
    result::TaskGroupResult,
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

    pub fn execute(&self) -> Result<TaskGroupResult> {
        let (reader, writer) = mk_pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                close_fd(reader.into_raw_fd())?;

                self.execution_child(writer)?;
                exit(0);
            }
            Ok(ForkResult::Parent { child }) => {
                close_fd(writer.into_raw_fd())?;

                let wait_result = waitpid(child, None);
                match wait_result {
                    Ok(status) => {
                        println!("Child process exited with status: {:?}", status);
                    }
                    Err(e) => {
                        eprintln!("Failed to wait for child process: {}", e);
                    }
                }

                let results: TaskGroupResult = match serde_json::from_reader(reader) {
                    Ok(results) => {
                        println!("✅ Successfully parsed results from child process");
                        results
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to parse results from child process: {}", e);
                        return Err(FaberError::ParseResult {
                            e,
                            details: "Failed to parse results from child process".to_string(),
                        });
                    }
                };

                Ok(results)
            }

            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    fn execution_child(&self, mut writer: PipeWriter) -> Result<()> {
        // Wrap container setup in error handling
        match self.container.setup() {
            Ok(_) => {
                println!("✅ Container setup successful");

                let task_group_result: TaskGroupResult =
                    vec![ExecutionStepResult::Single(TaskResult::Completed {
                        stdout: "Hello, world!".to_string(),
                        stderr: "".to_string(),
                        exit_code: 0,
                        stats: TaskResultStats::default(),
                    })];

                serde_json::to_writer(&mut writer, &task_group_result).map_err(|e| {
                    FaberError::ParseResult {
                        e,
                        details: "Failed to serialize task group result".to_string(),
                    }
                })?;

                println!("✅ Task execution completed successfully");
                self.container.cleanup()?;
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Container setup failed: {}", e);

                // Send error result back to parent
                let error_result: TaskGroupResult =
                    vec![ExecutionStepResult::Single(TaskResult::Failed {
                        error: format!("Container setup failed: {}", e),
                        stats: TaskResultStats::default(),
                    })];

                serde_json::to_writer(&mut writer, &error_result).unwrap_or_else(|_| {
                    eprintln!("Failed to serialize error result");
                });

                Err(e)
            }
        }
    }
}
