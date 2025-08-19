use nix::sched::CloneFlags;
use nix::unistd::{ForkResult, Pid, fork};

use std::io::Write;
use std::os::fd::IntoRawFd;
use std::path::PathBuf;
use std::process::{Command, exit};
use std::time::Instant;

use crate::TaskResult;
use crate::builder::RuntimeBuilder;

use crate::prelude::*;
use crate::types::{CgroupConfig, FilesystemConfig, Mount, RuntimeLimits, Task};
use crate::utils::{close_fd, mk_pipe};
use crate::{cgroup, environment};

/// High-level entry point for preparing an isolated environment and running tasks.
#[derive(Debug)]
pub struct Runtime {
    pub(crate) host_container_root: PathBuf,
    pub(crate) hostname: String,
    pub(crate) mounts: Vec<Mount>,
    pub(crate) work_dir: PathBuf,
    pub(crate) filesystem_config: FilesystemConfig,
    pub(crate) runtime_limits: RuntimeLimits,
    pub(crate) cgroup_config: CgroupConfig,
}

impl Runtime {
    /// Get a builder to configure and construct a [`Runtime`].
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn run(self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Validate tasks
        self.validate_tasks(&tasks)?;

        // Create the main faber cgroup hierarchy once per request
        eprintln!("[RUNTIME] Creating main faber cgroup hierarchy for request");
        cgroup::create_faber_cgroup_hierarchy()?;

        // Create pipe for task results
        let (results_reader, mut results_writer) = mk_pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                // Close write end of the pipe in parent
                close_fd(results_writer.into_raw_fd())?;

                // Read and deserialize results to completion first to avoid
                // potential pipe backpressure deadlocks
                let results: Vec<TaskResult> = match serde_json::de::from_reader(&results_reader) {
                    Ok(r) => r,
                    Err(e) => {
                        if e.is_eof() {
                            return Err(Error::ProcessManagement {
                                operation: "read results".to_string(),
                                pid: child.as_raw(),
                                details: e.to_string(),
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

                let _timeout_killed = crate::utils::wait_for_child_with_timeout(
                    child,
                    self.runtime_limits.kill_timeout_seconds,
                )?;

                environment::cleanup(&self.host_container_root)?;

                Ok(results)
            }
            Ok(ForkResult::Child) => {
                close_fd(results_reader.into_raw_fd())?;

                match self.run_in_manager(tasks) {
                    Ok(results) => {
                        serde_json::to_writer(&mut results_writer, &results).map_err(|e| {
                            Error::ProcessManagement {
                                operation: "write results".to_string(),
                                pid: -1,
                                details: format!("Failed to write results: {e}"),
                            }
                        })?;
                        exit(0);
                    }
                    Err(e) => {
                        eprintln!("Error in run_in_manager: {e}");
                        exit(1);
                    }
                }
            }
            Err(e) => Err(Error::ProcessManagement {
                operation: "fork process".to_string(),
                pid: -1,
                details: format!("Fork failed in parent process: {e:?}"),
            }),
        }
    }

    fn run_in_manager(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        self.setup_container_environment()?;
        self.execute_tasks(tasks)
    }

    /// Sets up the container environment with namespaces, mounts, and basic filesystem.
    fn setup_container_environment(&self) -> Result<()> {
        environment::create_container_root(&self.host_container_root)?;

        let unshare_flags = CloneFlags::CLONE_NEWNS // # mount
            | CloneFlags::CLONE_NEWUTS // # hostname
            | CloneFlags::CLONE_NEWIPC // # ipc
            | CloneFlags::CLONE_NEWNET; // # net

        environment::unshare(unshare_flags)?;
        environment::bind_mounts(&self.host_container_root, &self.mounts)?;
        environment::pivot_root_to(&self.host_container_root)?;
        environment::create_dev_devices()?;
        environment::create_proc()?;
        environment::create_sys()?;
        environment::create_cgroup()?;
        environment::create_work_dir(&self.work_dir, &self.filesystem_config.workdir_size)?;
        environment::create_tmp_dir(&self.work_dir, &self.filesystem_config.tmp_size)?;
        environment::set_container_hostname(&self.hostname)?;

        Ok(())
    }

    /// Executes all tasks and returns their results.
    fn execute_tasks(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        let mut results: Vec<TaskResult> = Vec::with_capacity(tasks.len());

        for task in tasks {
            if let Some(last_task) = results.last() {
                if last_task.exit_code != 0 {
                    results.push(self.create_skipped_result());
                    continue;
                }
            }

            let task_result = self.execute_single_task(task)?;

            results.push(task_result);
        }

        Ok(results)
    }

    /// Executes a single task and returns its result.
    fn execute_single_task(&self, task: Task) -> Result<TaskResult> {
        let (task_reader, task_writer) = mk_pipe()?;
        let start_time = Instant::now();

        // Create task-specific cgroup for this task
        eprintln!("[RUNTIME] Creating task-specific cgroup for task execution");

        let task_cgroup_path = cgroup::create_task_cgroup(&self.cgroup_config)?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                eprintln!("[RUNTIME] Parent process: child PID = {}", child.as_raw());
                close_fd(task_writer.into_raw_fd())?;

                let task_result =
                    self.handle_parent_process(child, task_reader, start_time, &task_cgroup_path)?;
                Ok(task_result)
            }
            Ok(ForkResult::Child) => {
                eprintln!("[RUNTIME] Child process starting");
                close_fd(task_reader.into_raw_fd())?;
                self.handle_child_process(task, task_writer, &task_cgroup_path)
            }
            Err(e) => Err(Error::ProcessManagement {
                operation: "fork per-task".to_string(),
                pid: -1,
                details: format!("Fork failed in task loop: {e:?}"),
            }),
        }
    }

    /// Handles the parent process side of task execution.
    fn handle_parent_process(
        &self,
        pid: Pid,
        task_reader: std::io::PipeReader,
        start_time: Instant,
        task_cgroup_path: &str,
    ) -> Result<TaskResult> {
        // Read TaskResult from child
        let mut task_result: TaskResult = match serde_json::de::from_reader(&task_reader) {
            Ok(res) => res,
            Err(e) => {
                return Err(Error::ProcessManagement {
                    operation: "read task result".to_string(),
                    pid: pid.as_raw(),
                    details: format!("Failed to read/deserialize task result: {e}"),
                });
            }
        };

        // Wait for per-task child to finish with timeout
        let timeout_killed = if let Some(timeout) = self.runtime_limits.kill_timeout_seconds {
            eprintln!(
                "[RUNTIME] Setting up timeout monitoring for {} seconds",
                timeout
            );
            let timeout_duration = std::time::Duration::from_secs(timeout);
            let start_time = std::time::Instant::now();

            // Spawn a monitoring thread to kill the child after timeout
            let pid_clone = pid;
            let timeout_handle = std::thread::spawn(move || {
                std::thread::sleep(timeout_duration);
                eprintln!(
                    "[RUNTIME] Timeout reached, killing process {}",
                    pid_clone.as_raw()
                );
                let kill_result =
                    nix::sys::signal::kill(pid_clone, nix::sys::signal::Signal::SIGKILL);
                match kill_result {
                    Ok(_) => eprintln!(
                        "[RUNTIME] Successfully sent SIGKILL to process {}",
                        pid_clone.as_raw()
                    ),
                    Err(e) => eprintln!(
                        "[RUNTIME] Failed to send SIGKILL to process {}: {:?}",
                        pid_clone.as_raw(),
                        e
                    ),
                }
            });

            // Wait for the child to exit with a shorter timeout to check periodically
            let mut timeout_occurred = false;
            let check_interval = std::time::Duration::from_millis(100); // Check every 100ms

            loop {
                // Try to wait for the child with a short timeout
                match nix::sys::wait::waitpid(pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG)) {
                    Ok(nix::sys::wait::WaitStatus::Exited(_, _)) => {
                        eprintln!("[RUNTIME] Child process {} exited normally", pid.as_raw());
                        break;
                    }
                    Ok(nix::sys::wait::WaitStatus::Signaled(_, signal, _)) => {
                        eprintln!(
                            "[RUNTIME] Child process {} was killed by signal {:?}",
                            pid.as_raw(),
                            signal
                        );
                        if signal == nix::sys::signal::Signal::SIGKILL {
                            timeout_occurred = true;
                        }
                        break;
                    }
                    Ok(nix::sys::wait::WaitStatus::StillAlive) => {
                        // Process is still running, check if we've exceeded timeout
                        if start_time.elapsed() >= timeout_duration {
                            eprintln!(
                                "[RUNTIME] Timeout exceeded, waiting for kill to take effect"
                            );
                            timeout_occurred = true;
                            // Give the kill signal a moment to take effect
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            break;
                        }
                        // Wait a bit before checking again
                        std::thread::sleep(check_interval);
                    }
                    Ok(nix::sys::wait::WaitStatus::Stopped(_, _)) => {
                        // Process stopped, continue monitoring
                        std::thread::sleep(check_interval);
                    }
                    Ok(nix::sys::wait::WaitStatus::PtraceEvent(_, _, _)) => {
                        // Ptrace event, continue monitoring
                        std::thread::sleep(check_interval);
                    }
                    Ok(nix::sys::wait::WaitStatus::PtraceSyscall(_)) => {
                        // Ptrace syscall, continue monitoring
                        std::thread::sleep(check_interval);
                    }
                    Ok(nix::sys::wait::WaitStatus::Continued(_)) => {
                        // Process continued, continue monitoring
                        std::thread::sleep(check_interval);
                    }
                    Err(e) => {
                        eprintln!(
                            "[RUNTIME] Error waiting for child process {}: {:?}",
                            pid.as_raw(),
                            e
                        );
                        break;
                    }
                }
            }

            // Cancel the timeout thread
            drop(timeout_handle);

            // Final wait to ensure the process is gone
            if timeout_occurred {
                // Wait a bit more for the process to actually exit
                let _ = nix::sys::wait::waitpid(pid, None);
            }

            timeout_occurred
        } else {
            // No timeout configured, wait normally
            eprintln!(
                "[RUNTIME] No timeout configured, waiting for child process {} to exit",
                pid.as_raw()
            );
            crate::utils::wait_for_child(pid)?;
            false
        };

        // If the task was killed due to timeout, return an error
        if timeout_killed {
            return Err(Error::ProcessManagement {
                operation: "task execution".to_string(),
                pid: pid.as_raw(),
                details: format!(
                    "Task killed due to timeout ({} seconds)",
                    self.runtime_limits.kill_timeout_seconds.unwrap_or(0)
                ),
            });
        }

        // Read task statistics from the task cgroup before cleanup
        let task_stats = match cgroup::read_task_stats(&task_cgroup_path) {
            Ok(stats) => {
                eprintln!("[RUNTIME] Successfully read task statistics from cgroup");
                stats
            }
            Err(e) => {
                eprintln!("[RUNTIME] Warning: Failed to read task stats: {}", e);
                cgroup::TaskStats::default()
            }
        };

        // Clean up the task cgroup directory
        if let Err(e) = cgroup::cleanup_task_cgroup(&task_cgroup_path) {
            eprintln!("[RUNTIME] Warning: Failed to cleanup task cgroup: {}", e);
        }

        // Populate metrics including CPU and memory statistics from cgroup
        task_result.execution_time_ms = Some(start_time.elapsed().as_millis() as u64);
        task_result.cpu_usage_usec = task_stats.cpu.usage_usec;
        task_result.cpu_user_usec = task_stats.cpu.user_usec;
        task_result.cpu_system_usec = task_stats.cpu.system_usec;
        task_result.memory_current_bytes = Some(task_stats.memory.current);
        task_result.memory_peak_bytes = Some(task_stats.memory.peak);
        task_result.memory_limit_bytes = Some(task_stats.memory.max);
        task_result.pids_current = Some(task_stats.pids.current);
        task_result.pids_max = Some(task_stats.pids.max);

        Ok(task_result)
    }

    /// Handles the child process side of task execution.
    fn handle_child_process(
        &self,
        task: Task,
        mut task_writer: std::io::PipeWriter,
        task_cgroup_path: &str,
    ) -> ! {
        // Set up task-specific namespaces
        let flags = CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNS;
        if let Err(e) = environment::unshare(flags) {
            let result = self.create_error_result("unshare failed", &e.to_string());
            self.write_result_and_exit(&mut task_writer, &result);
        }

        // Add this child process to the task cgroup
        let current_pid = std::process::id();
        if let Err(e) = cgroup::add_process_to_task_cgroup(task_cgroup_path, current_pid) {
            let result = self.create_error_result("add process to cgroup failed", &e.to_string());
            self.write_result_and_exit(&mut task_writer, &result);
        }

        // Write files if specified
        if let Some(files) = &task.files {
            if let Err(e) = environment::write_files(&self.work_dir, files) {
                let result = self.create_error_result("write files failed", &e.to_string());
                self.write_result_and_exit(&mut task_writer, &result);
            }
        }

        // Prepare and spawn the command
        let child = match self.prepare_and_spawn_command(&task, &mut task_writer) {
            Ok(child) => child,
            Err(_) => exit(0), // Error already written to pipe
        };

        // Wait for command to finish and get result
        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                let result = self.create_error_result("wait failed", &e.to_string());
                self.write_result_and_exit(&mut task_writer, &result);
            }
        };

        // Parse the command output
        let result = TaskResult::from(output);

        // Write the result to the pipe and exit
        self.write_result_and_exit(&mut task_writer, &result);
    }

    /// Prepares and spawns a command for execution.
    fn prepare_and_spawn_command(
        &self,
        task: &Task,
        task_writer: &mut std::io::PipeWriter,
    ) -> Result<std::process::Child> {
        let mut cmd = Command::new(&task.cmd);
        cmd.current_dir(&self.work_dir);

        if let Some(args) = &task.args {
            cmd.args(args);
        }

        cmd.env_clear();

        if let Some(env) = &task.env {
            cmd.envs(env);
        }

        let has_path = cmd.get_envs().any(|(key, _)| key == "PATH");
        if !has_path {
            cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
        }

        if task.stdin.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        } else {
            cmd.stdin(std::process::Stdio::null());
        }

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Drop privileges to nobody user for security before spawning the command
        // if let Err(e) = environment::drop_privileges_to_nobody() {
        //     let result = self.create_error_result("failed to drop privileges", &e.to_string());
        //     self.write_result_and_exit(task_writer, &result);
        // }

        cmd.spawn().map_err(|e| {
            let result = self.create_error_result("spawn failed", &e.to_string());
            self.write_result_and_exit(task_writer, &result);
            // This error is never returned since write_result_and_exit calls exit(0)
            unreachable!("write_result_and_exit should have called exit(0)")
        })
    }

    /// Creates a TaskResult for skipped tasks.
    fn create_skipped_result(&self) -> TaskResult {
        TaskResult {
            stdout: String::new(),
            stderr: "skipped: previous task failed".to_string(),
            exit_code: -1,
            execution_time_ms: None,
            cpu_usage_usec: None,
            cpu_user_usec: None,
            cpu_system_usec: None,
            memory_peak_bytes: None,
            memory_current_bytes: None,
            memory_limit_bytes: None,
            pids_current: None,
            pids_max: None,
        }
    }

    /// Creates a TaskResult for error conditions.
    fn create_error_result(&self, operation: &str, details: &str) -> TaskResult {
        TaskResult {
            stdout: String::new(),
            stderr: format!("{operation}: {details}"),
            exit_code: -1,
            execution_time_ms: None,
            cpu_usage_usec: None,
            cpu_user_usec: None,
            cpu_system_usec: None,
            memory_peak_bytes: None,
            memory_current_bytes: None,
            memory_limit_bytes: None,
            pids_current: None,
            pids_max: None,
        }
    }

    /// Writes a result to the task writer and exits the process.
    fn write_result_and_exit(
        &self,
        task_writer: &mut std::io::PipeWriter,
        result: &TaskResult,
    ) -> ! {
        let _ = serde_json::to_writer(&mut *task_writer, result);
        let _ = task_writer.flush();
        exit(0);
    }

    /// Basic validation for task lists.
    fn validate_tasks(&self, tasks: &[Task]) -> Result<()> {
        if tasks.is_empty() {
            return Err(Error::Validation {
                field: "tasks".to_string(),
                details: "Task list cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}
