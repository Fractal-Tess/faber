use nix::sys::wait::waitpid;
use nix::unistd::{ForkResult, fork};

use std::io::{Read, Write};
use std::os::fd::IntoRawFd;
use std::process::{Command, Stdio, exit};

use crate::environment::ContainerEnvironment;
use crate::prelude::*;
use crate::utils::{close_fd, mk_pipe};
use crate::{Task, TaskResult};

pub struct Executor {
    pub(crate) tasks: Vec<Task>,
    pub(crate) env: ContainerEnvironment,
}

impl Executor {
    /// Prepares the execution environment
    pub fn prepare(&self) -> Result<()> {
        self.env.prepare_pre_pid_namespace()?;

        Ok(())
    }

    /// Runs the tasks in the execution environment
    pub fn run(&self) -> Result<Vec<TaskResult>> {
        let (mut results_reader, mut results_writter) = mk_pipe()?;

        self.env.prepare_pre_pid_namespace()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                close_fd(results_writter.into_raw_fd())?;

                let mut serilized_results = String::new();
                results_reader
                    .read_to_string(&mut serilized_results)
                    .expect("Failed to read results");

                let results: Vec<TaskResult> = serde_json::from_str(&serilized_results)
                    .expect("Failed to deserialize results");

                let _ = waitpid(child, None).map_err(|e| Error::ProcessManagement {
                    operation: "wait for child".to_string(),
                    pid: child.as_raw(),
                    details: format!("Failed to wait for child: {e}"),
                })?;

                Ok(results)
            }
            Ok(ForkResult::Child) => {
                close_fd(results_reader.into_raw_fd())?;

                let results = self.run_in_execution_environment()?;
                let serilized_results =
                    serde_json::to_string(&results).expect("Failed to serialize results");

                results_writter
                    .write_all(serilized_results.as_bytes())
                    .map_err(|e| Error::ProcessManagement {
                        operation: "write results".to_string(),
                        pid: -1,
                        details: format!("Failed to write results: {e}"),
                    })?;

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

    fn run_in_execution_environment(&self) -> Result<Vec<TaskResult>> {
        self.env.prepare_post_pid_namespace()?;

        let mut results: Vec<TaskResult> = Vec::with_capacity(self.tasks.len());

        for t in &self.tasks {
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
            results.push(result);
        }

        Ok(results)
    }
}
