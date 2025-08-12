use nix::sys::wait::waitpid;
use nix::unistd::{ForkResult, fork};

use std::io::{Read, Write};
use std::os::fd::IntoRawFd;
use std::process::{Command, Stdio, exit};

use crate::environment::ContainerEnvironment;
use crate::prelude::*;
use crate::utils::{close_fd, mk_pipe};
use crate::{Task, TaskResult};
use tracing::debug;

/// Executes a list of tasks inside a prepared, isolated environment.
///
/// This type is typically constructed and driven by [`Runtime`]. It owns the
/// configured [`ContainerEnvironment`] and coordinates the pre/post namespace
/// preparation steps around the actual command execution.
pub struct Executor {
    pub(crate) tasks: Vec<Task>,
    pub(crate) env: ContainerEnvironment,
}

impl Executor {
    /// Prepares the execution environment prior to entering the PID namespace.
    ///
    /// This sets up container root, namespaces, bind mounts, pivot root,
    /// devices, and workdir, but does not yet create `/proc`, `/sys`, `/tmp`.
    pub fn prepare(&self) -> Result<()> {
        debug!("Executor::prepare: begin");
        self.env.prepare_pre_pid_namespace()?;
        debug!("Executor::prepare: done");

        Ok(())
    }

    /// Runs the tasks in the execution environment.
    ///
    /// Forks a child that executes in the isolated PID namespace, serializes
    /// results, and passes them to the parent via a pipe.
    pub fn run(&self) -> Result<Vec<TaskResult>> {
        debug!("Executor::run: begin");
        let (mut results_reader, mut results_writter) = mk_pipe()?;
        debug!("Executor::run: pipe created, forking");

        self.env.prepare_pre_pid_namespace()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                debug!(pid = child.as_raw(), "Executor::run[parent]: forked child");
                close_fd(results_writter.into_raw_fd())?;

                let mut serilized_results = String::new();
                results_reader
                    .read_to_string(&mut serilized_results)
                    .expect("Failed to read results");
                debug!(
                    bytes = serilized_results.len(),
                    "Executor::run[parent]: read results"
                );

                let results: Vec<TaskResult> = serde_json::from_str(&serilized_results)
                    .expect("Failed to deserialize results");

                let _ = waitpid(child, None).map_err(|e| Error::ProcessManagement {
                    operation: "wait for child".to_string(),
                    pid: child.as_raw(),
                    details: format!("Failed to wait for child: {e}"),
                })?;
                debug!("Executor::run[parent]: child joined, returning results");

                Ok(results)
            }
            Ok(ForkResult::Child) => {
                debug!("Executor::run[child]: in child");
                close_fd(results_reader.into_raw_fd())?;

                let results = self.run_in_execution_environment()?;
                debug!(
                    result_count = results.len(),
                    "Executor::run[child]: tasks finished"
                );
                let serilized_results =
                    serde_json::to_string(&results).expect("Failed to serialize results");

                results_writter
                    .write_all(serilized_results.as_bytes())
                    .map_err(|e| Error::ProcessManagement {
                        operation: "write results".to_string(),
                        pid: -1,
                        details: format!("Failed to write results: {e}"),
                    })?;

                debug!("Executor::run[child]: exiting child");
                exit(0);
            }
            Err(e) => {
                return Err(Error::ProcessManagement {
                    operation: "fork".to_string(),
                    pid: -1,
                    details: format!("Failed to fork: {e}"),
                });
            }
        }
    }

    /// Executes inside the isolated PID namespace and returns collected results.
    fn run_in_execution_environment(&self) -> Result<Vec<TaskResult>> {
        debug!("Executor::run_in_execution_environment: preparing post-pid namespace");
        self.env.prepare_post_pid_namespace()?;

        let mut results: Vec<TaskResult> = Vec::with_capacity(self.tasks.len());

        for (idx, t) in self.tasks.iter().enumerate() {
            debug!(task_index = idx, cmd = t.cmd, "Executor: starting task");
            // If files are provided, write them to the workdir
            if let Some(files) = &t.files {
                self.env.write_files_to_workdir(files).map_err(|e| {
                    Error::ContainerEnvironment {
                        operation: "write files to workdir".to_string(),
                        details: format!("Failed to write task files: {e}"),
                    }
                })?;
            }

            // Main command
            let mut cmd = Command::new(&t.cmd);

            // Arguments
            if let Some(args) = &t.args {
                cmd.args(args);
            }

            // Env
            cmd.env_clear();
            if let Some(env) = &t.env {
                cmd.envs(env);
            }
            // Check if PATH is set in the command's environment, otherwise add a minimal PATH
            let has_path = cmd.get_envs().any(|(key, _)| key == "PATH");
            if !has_path {
                cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
            }

            // Current working directory
            if let Some(dir) = &t.cwd {
                cmd.current_dir(dir);
            }

            // Set stdout and stderr to piped
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            // Run the command
            let output = cmd.output().map_err(|e| Error::ProcessManagement {
                operation: "run task".to_string(),
                pid: -1,
                details: format!("Failed to run task: {e}"),
            })?;

            // Create task result
            let result = TaskResult::from(output);
            debug!(
                task_index = idx,
                exit_code = result.exit_code,
                "Executor: task finished"
            );
            results.push(result);
        }

        Ok(results)
    }
}
