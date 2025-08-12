use nix::unistd::{ForkResult, fork};

use std::io::Write;
use std::os::fd::IntoRawFd;
use std::process::exit;

use crate::TaskResult;
use crate::builder::RuntimeBuilder;
use crate::environment::ContainerEnvironment;
use crate::executor::Executor;
use crate::prelude::*;
use crate::types::Task;
use crate::utils::{close_fd, mk_pipe, wait_for_child};

#[derive(Debug)]
pub struct Runtime {
    pub(crate) env: ContainerEnvironment,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn run(self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Validate tasks
        self.validate_tasks(&tasks)?;

        // Create pipe for task results
        let (results_reader, mut results_writter) = mk_pipe()?;

        // Fork
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                // Close write end of the pipe
                close_fd(results_writter.into_raw_fd())?;

                // Wait for child to exit
                wait_for_child(child)?;
            }
            Ok(ForkResult::Child) => {
                // Close read end of the pipe
                close_fd(results_reader.into_raw_fd())?;

                // Create executor
                let executor = Executor {
                    tasks,
                    env: self.env,
                };

                // Prepare the execution environment
                executor.prepare()?;

                // Run the tasks
                let results = executor.run()?;

                // Serialize results
                let serilized_results =
                    serde_json::to_string(&results).expect("Failed to serialize results");

                // Write task results to parent
                results_writter
                    .write_all(serilized_results.as_bytes())
                    .map_err(|e| Error::ProcessManagement {
                        operation: "write results".to_string(),
                        pid: -1,
                        details: format!("Failed to write results: {e}"),
                    })?;

                // Exit child
                exit(0);
            }
            Err(e) => {
                return Err(Error::ProcessManagement {
                    operation: "fork process".to_string(),
                    pid: -1,
                    details: format!("Fork failed in parent process: {e:?}"),
                });
            }
        };

        // Deserialize results
        let results: Vec<TaskResult> =
            serde_json::de::from_reader(&results_reader).expect("Failed to deserialize results");

        // Cleanup environment
        self.env.cleanup()?;

        Ok(results)
    }

    fn validate_tasks(&self, tasks: &[Task]) -> Result<()> {
        // Ensure tasks are not empty
        if tasks.is_empty() {
            return Err(Error::Validation {
                field: "tasks".to_string(),
                details: "Task list cannot be empty".to_string(),
            });
        }

        // TODO: Additional validation
        Ok(())
    }
}
