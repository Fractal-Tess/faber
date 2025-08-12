use std::process::{Command, Stdio};

use crate::environment::ContainerEnvironment;
use crate::prelude::*;
use crate::{Task, TaskResult};
use tracing::{debug, trace};

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
        trace!("Executor::prepare: done");

        Ok(())
    }

    /// Runs the tasks in the execution environment without additional forking.
    ///
    /// Assumes the caller has already forked into the PID namespace. This will
    /// complete post-PID setup and execute tasks inline, returning results to
    /// the caller.
    pub fn run(&self) -> Result<Vec<TaskResult>> {
        debug!("Executor::run (inline): begin");
        let results = self.run_in_execution_environment()?;
        debug!(result_count = results.len(), "Executor::run (inline): done");
        trace!("Executor::run (inline): returning results");
        Ok(results)
    }

    /// Executes inside the isolated PID namespace and returns collected results.
    fn run_in_execution_environment(&self) -> Result<Vec<TaskResult>> {
        trace!("Executor::run_in_execution_environment: preparing post-pid namespace");
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
