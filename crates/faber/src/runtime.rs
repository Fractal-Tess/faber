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
use tracing::{debug, trace};

/// High-level entry point for preparing an isolated environment and running tasks.
#[derive(Debug)]
pub struct Runtime {
    pub(crate) env: ContainerEnvironment,
}

impl Runtime {
    /// Get a builder to configure and construct a [`Runtime`].
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    /// Run a sequence of tasks within a prepared, isolated environment.
    ///
    /// This method forks a child process to own the isolated namespaces and
    /// uses a pipe to shuttle serialized task results back to the parent.
    /// The parent reads results to completion, then waits for the child to exit
    /// and performs environment cleanup.
    ///
    /// # Errors
    /// Returns an error if validation fails, process management calls fail,
    /// or if serialization/deserialization encounters issues.
    pub fn run(self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        debug!(task_count = tasks.len(), "Runtime::run: start");
        // Validate tasks
        self.validate_tasks(&tasks)?;
        trace!("Runtime::run: validation ok");

        // Create pipe for task results
        let (results_reader, mut results_writer) = mk_pipe()?;
        trace!("Runtime::run: pipe created");

        // Fork
        debug!("Runtime::run: forking child");
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                debug!(pid = child.as_raw(), "Runtime::run[parent]: forked child");
                // Close write end of the pipe in parent
                close_fd(results_writer.into_raw_fd())?;
                trace!("Runtime::run[parent]: closed write end, reading results");

                // Read and deserialize results to completion first to avoid
                // potential pipe backpressure deadlocks
                let results: Vec<TaskResult> = match serde_json::de::from_reader(&results_reader) {
                    Ok(r) => r,
                    Err(e) => {
                        if e.is_eof() {
                            return Err(Error::ProcessManagement {
                                operation: "read results".to_string(),
                                pid: child.as_raw(),
                                details: "EOF before any results were written by child".to_string(),
                            });
                        } else {
                            return Err(Error::ProcessManagement {
                                operation: "read results".to_string(),
                                pid: child.as_raw(),
                                details: format!("Failed to read/deserialize results: {e}"),
                            });
                        }
                    }
                };
                debug!(
                    result_count = results.len(),
                    "Runtime::run[parent]: results received, waiting for child"
                );

                // Wait for child to exit
                wait_for_child(child)?;
                trace!("Runtime::run[parent]: child exited");

                // Cleanup environment
                self.env.cleanup()?;
                debug!("Runtime::run[parent]: cleanup done, returning results");

                Ok(results)
            }
            Ok(ForkResult::Child) => {
                trace!("Runtime::run[child]: in child, closing read end");
                // Close read end of the pipe in child
                close_fd(results_reader.into_raw_fd())?;

                // Create executor
                let executor = Executor {
                    tasks,
                    env: self.env,
                };

                // Prepare the execution environment
                debug!("Runtime::run[child]: preparing execution environment");
                executor.prepare()?;
                trace!("Runtime::run[child]: prepare done, running tasks");

                // Run the tasks
                let results = executor.run()?;
                trace!(
                    result_count = results.len(),
                    "Runtime::run[child]: tasks finished, serializing"
                );

                // Serialize results
                let serialized_results =
                    serde_json::to_string(&results).expect("Failed to serialize results");

                // Write task results to parent
                trace!(
                    bytes = serialized_results.len(),
                    "Runtime::run[child]: writing results to pipe"
                );
                results_writer
                    .write_all(serialized_results.as_bytes())
                    .map_err(|e| Error::ProcessManagement {
                        operation: "write results".to_string(),
                        pid: -1,
                        details: format!("Failed to write results: {e}"),
                    })?;

                // Exit child
                trace!("Runtime::run[child]: exiting");
                exit(0);
            }
            Err(e) => {
                return Err(Error::ProcessManagement {
                    operation: "fork process".to_string(),
                    pid: -1,
                    details: format!("Fork failed in parent process: {e:?}"),
                });
            }
        }
    }

    /// Basic validation for task lists.
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
