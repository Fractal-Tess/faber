use nix::{
    sys::{
        signal::{Signal, kill},
        wait::waitpid,
    },
    unistd::{ForkResult, Pid, close, fork, pipe},
};

use crate::{
    TaskResult,
    builder::RuntimeBuilder,
    cgroup::CgroupManager,
    environment::ContainerEnvironment,
    prelude::*,
    types::{RuntimeLimits, Task},
};

use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{FromRawFd, IntoRawFd, OwnedFd},
        unix::process::ExitStatusExt,
    },
    path::Path,
    process::{Command, Stdio, exit},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Debug)]
pub struct Runtime {
    pub(crate) id: String,
    pub(crate) env: ContainerEnvironment,
    pub(crate) limits: RuntimeLimits,
    pub(crate) cgroup_manager: CgroupManager,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn run(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Validate tasks
        if tasks.is_empty() {
            return Err(Error::Validation {
                field: "tasks".to_string(),
                details: "Task list cannot be empty".to_string(),
            });
        }

        // Create pipe for task results
        let (results_read_fd, results_write_fd) = pipe()?;

        // Fork
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                // Close write end of the pipe
                close(results_write_fd)?;

                // Read task results from child
                let results_json = self.read_all_from_fd(results_read_fd);

                // Wait for child to exit
                waitpid(child, None).map_err(|e| Error::ProcessManagement {
                    operation: "wait for child process".to_string(),
                    pid: child.as_raw(),
                    details: format!("waitpid failed: {e}"),
                })?;

                // Spawn killer thread
                let timeout_secs = self.limits.kill_timeout_seconds.unwrap_or(10);
                let (killer_handle, cancel_kill) = Self::spawn_killer(child, timeout_secs);
                cancel_kill.store(true, Ordering::SeqCst);
                killer_handle.join().map_err(|e| Error::ThreadManagement {
                    operation: "join killer thread".to_string(),
                    details: format!("failed to join killer thread: {e:?}"),
                })?;

                // Task results
                let task_results = Self::deserialize_results(&results_json)?;

                // Cleanup cgroup
                self.cgroup_manager.cleanup()?;

                // Cleanup environment
                self.env.cleanup()?;

                Ok(task_results)
            }
            Ok(ForkResult::Child) => {
                // Run tasks in child
                let results_vec = match self.run_tasks_in_child(tasks) {
                    Ok(v) => v,
                    Err(e) => {
                        vec![TaskResult {
                            stdout: String::new(),
                            stderr: format!("setup failed: {e:?}"),
                            exit_code: 1,
                        }]
                    }
                };

                // Write task results to parent
                self.write_child_results(results_write_fd, &results_vec);

                // Exit child
                exit(0);
            }
            Err(e) => {
                // Cleanup cgroup
                self.cgroup_manager.cleanup()?;

                // Cleanup environment
                self.env.cleanup()?;

                Err(Error::ProcessManagement {
                    operation: "fork process".to_string(),
                    pid: -1,
                    details: format!("Fork failed in parent process: {e:?}"),
                })
            }
        }
    }

    fn spawn_killer(child: Pid, timeout_secs: u64) -> (JoinHandle<()>, Arc<AtomicBool>) {
        let cancel_kill = Arc::new(AtomicBool::new(false));
        let cancel_kill_for_thread = cancel_kill.clone();
        let killer_handle = thread::spawn(move || {
            let timeout = Duration::from_secs(timeout_secs);
            let start = Instant::now();
            while start.elapsed() < timeout {
                if cancel_kill_for_thread.load(Ordering::SeqCst) {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
            if !cancel_kill_for_thread.load(Ordering::SeqCst) {
                let _ = kill(child, Signal::SIGKILL);
            }
        });
        (killer_handle, cancel_kill)
    }

    fn read_all_from_fd(&self, fd: OwnedFd) -> String {
        let mut reader = unsafe { File::from_raw_fd(fd.into_raw_fd()) };
        let mut s = String::new();
        if let Err(e) = reader.read_to_string(&mut s) {
            // Log the error but return empty string to avoid panicking
            eprintln!("Warning: Failed to read from file descriptor: {e}");
        }
        s
    }

    fn deserialize_results(json: &str) -> Result<Vec<TaskResult>> {
        serde_json::from_str(json).map_err(|e| Error::Deserialization {
            operation: "parse child results".to_string(),
            data_type: "Vec<TaskResult>".to_string(),
            details: format!(
                "JSON parsing failed: {e}. Raw data (first 256 chars): {}",
                json.chars().take(256).collect::<String>()
            ),
        })
    }

    fn write_child_results(&self, write_fd: OwnedFd, results: &Vec<TaskResult>) {
        let mut writer = unsafe { File::from_raw_fd(write_fd.into_raw_fd()) };
        let serialized = serde_json::to_string(results)
            .map_err(|e| Error::Serialization {
                operation: "serialize task results".to_string(),
                data_type: "Vec<TaskResult>".to_string(),
                details: format!("Failed to serialize results: {e}"),
            })
            .unwrap_or_else(|_| String::from("[]"));

        if let Err(e) = writer.write_all(serialized.as_bytes()) {
            eprintln!("Warning: Failed to write results to file descriptor: {e}");
        }
        if let Err(e) = writer.flush() {
            eprintln!("Warning: Failed to flush results to file descriptor: {e}");
        }
    }

    // Run tasks in child 2st child process
    fn run_tasks_in_child(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Set up namespaces and container root
        self.env
            .initialize()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "initialize container environment".to_string(),
                details: format!("Failed to initialize container: {e}"),
            })?;

        // Create pipe for task results
        let (read_fd, write_fd) = pipe()?;

        // Fork
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                self.cgroup_manager.add_proc(child.as_raw())?;
                // Parent of PID 1: close write end and read Vec<TaskResult> JSON
                close(write_fd)?;
                let json = self.read_all_from_fd(read_fd);
                let _ = waitpid(child, None).map_err(|e| Error::ProcessManagement {
                    operation: "wait for PID1 process".to_string(),
                    pid: child.as_raw(),
                    details: format!("waitpid failed: {e}"),
                })?;
                let results: Vec<TaskResult> = Self::deserialize_results(&json)?;

                Ok(results)
            }
            Ok(ForkResult::Child) => {
                // This becomes PID 1 in the new PID namespace
                close(read_fd)?;
                let results = self.run_tasks(tasks)?;
                self.write_child_results(write_fd, &results);
                exit(0);
            }
            Err(e) => Err(Error::ProcessManagement {
                operation: "fork PID1 process".to_string(),
                pid: -1,
                details: format!("Failed to fork PID1 for task runner: {e:?}"),
            }),
        }
    }

    // Run tasks in child 3st child process
    fn run_tasks(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Tasks results
        let mut all_results: Vec<TaskResult> = Vec::with_capacity(tasks.len());

        // Run tasks
        for task in tasks.into_iter() {
            // If files are provided, write them to the workdir
            if let Some(files) = &task.files {
                self.env.write_files_to_workdir(files).map_err(|e| {
                    Error::ContainerEnvironment {
                        operation: "write files to workdir".to_string(),
                        details: format!("Failed to write task files: {e}"),
                    }
                })?;
            }

            // Create command
            let mut cmd = Command::new(&task.cmd);

            // Add arguments
            if let Some(args) = &task.args {
                cmd.args(args);
            }

            // Add environment variables
            if let Some(env) = &task.env {
                cmd.envs(env.iter());

                // Add PATH if not set
                if !env.contains_key("PATH") {
                    cmd.env(
                        "PATH",
                        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                    );
                }
            } else {
                // Add PATH if not set
                cmd.env(
                    "PATH",
                    "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                );
            }

            // Set current directory

            if let Some(cwd) = &task.cwd {
                // If cwd is provided, set it
                if !cwd.is_empty() && Path::new(cwd).exists() {
                    cmd.current_dir(cwd);
                }
            } else {
                // If cwd is not provided, set it to the workdir
                cmd.current_dir(&self.env.work_dir);
            }

            // Set stdout and stderr to piped
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            // Execute command
            match cmd.output() {
                Ok(output) => all_results.push(TaskResult {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: output.status.code().unwrap_or_else(|| {
                        if let Some(sig) = output.status.signal() {
                            128 + sig
                        } else {
                            1
                        }
                    }),
                }),
                Err(e) => all_results.push(TaskResult {
                    stdout: String::new(),
                    stderr: format!("failed to execute '{}': {e}", task.cmd),
                    exit_code: 1,
                }),
            }
        }
        Ok(all_results)
    }
}
