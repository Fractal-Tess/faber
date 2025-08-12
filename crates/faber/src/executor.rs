use std::os::fd::IntoRawFd;
use std::process::{Command, Stdio};

use crate::environment::ContainerEnvironment;
use crate::prelude::*;
use crate::utils::{close_fd, mk_pipe};
use crate::{Task, TaskResult};
use nix::sys::wait::waitpid;
use nix::unistd::{ForkResult, fork};
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
        // let results = self.run_in_execution_environment()?;
        let (result_reader, result_writer) = mk_pipe()?;

        match unsafe { fork()? } {
            ForkResult::Child => {
                // Close write end of the pipe in child
                close_fd(result_reader.into_raw_fd())?;

                // Prepare the environment
                ContainerEnvironment::pre_exec()?;

                let mut results: Vec<TaskResult> = Vec::with_capacity(self.tasks.len());

                for t in &self.tasks {
                    let result = self.execute_task(t)?;
                    results.push(result);
                }

                // Serialize results to the pipe
                serde_json::ser::to_writer(result_writer, &results).map_err(|e| {
                    Error::ProcessManagement {
                        operation: "serialize results".to_string(),
                        pid: -1,
                        details: format!("Failed to serialize results: {e}"),
                    }
                })?;

                std::process::exit(0);
            }
            ForkResult::Parent { child } => {
                // Close read end of the pipe in parent
                close_fd(result_writer.into_raw_fd())?;

                // Read results from the pipe
                let results: Vec<TaskResult> = serde_json::de::from_reader(&result_reader)
                    .map_err(|e| Error::ProcessManagement {
                        operation: "deserialize results".to_string(),
                        pid: -1,
                        details: format!("Failed to deserialize results: {e}"),
                    })?;

                // TODO: Add child to cgroup
                waitpid(child, None)?;
                debug!("Executor::run (inline): child exited");

                Ok(results)
            }
        }
    }

    /// Executes inside the isolated PID namespace and returns collected results.
    fn execute_task(&self, task: &Task) -> Result<TaskResult> {
        // If files are provided, write them to the workdir
        if let Some(files) = &task.files {
            self.env
                .write_files_to_workdir(files)
                .map_err(|e| Error::ContainerEnvironment {
                    operation: "write files to workdir".to_string(),
                    details: format!("Failed to write task files: {e}"),
                })?;
        }

        // Main command
        let mut cmd = Command::new(&task.cmd);

        // Arguments
        if let Some(args) = &task.args {
            cmd.args(args);
        }

        // Env
        cmd.env_clear();
        if let Some(env) = &task.env {
            cmd.envs(env);
        }
        // Check if PATH is set in the command's environment, otherwise add a minimal PATH
        let has_path = cmd.get_envs().any(|(key, _)| key == "PATH");
        if !has_path {
            cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
        }

        // Current working directory
        if let Some(dir) = &task.cwd {
            cmd.current_dir(dir);
        } else {
            cmd.current_dir(&self.env.work_dir);
        }

        // Set up stdio: pipe stdout/stderr, and optionally stdin
        if task.stdin.is_some() {
            cmd.stdin(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Run the command
        let mut child = cmd.spawn().map_err(|e| Error::ProcessManagement {
            operation: "spawn task".to_string(),
            pid: -1,
            details: format!("Failed to spawn task '{}': {e}", &task.cmd),
        })?;

        // If stdin content provided, write it then drop the handle
        if let Some(input) = &task.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                stdin
                    .write_all(input.as_bytes())
                    .map_err(|e| Error::ProcessManagement {
                        operation: "write stdin".to_string(),
                        pid: -1,
                        details: format!("Failed to write stdin for '{}': {e}", &task.cmd),
                    })?;
            }
        }

        // Wait for completion and collect output
        let output = child
            .wait_with_output()
            .map_err(|e| Error::ProcessManagement {
                operation: "wait for task".to_string(),
                pid: -1,
                details: format!("Failed to wait for task '{}': {e}", &task.cmd),
            })?;

        // Create task result
        let result = TaskResult::from(output);

        Ok(result)
    }
}
