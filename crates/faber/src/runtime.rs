use nix::sched::CloneFlags;
use nix::sys::wait::WaitStatus;
use nix::unistd::{ForkResult, Pid, fork};

use std::io::{Read, Write};
use std::os::fd::IntoRawFd;
use std::path::PathBuf;
use std::process::{Command, exit};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};

use crate::TaskResult;
use crate::builder::RuntimeBuilder;

use crate::prelude::*;
use crate::types::{CgroupConfig, FilesystemConfig, Mount, Task};
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
    pub(crate) cgroup_config: CgroupConfig,
}

impl Runtime {
    /// Get a builder to configure and construct a [`Runtime`].
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn run(self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Validate tasks
        eprintln!("[DEBUG] Starting runtime with {} tasks", tasks.len());
        self.validate_tasks(&tasks)?;

        // Create the main faber cgroup hierarchy once per request
        eprintln!("[DEBUG] Creating main faber cgroup hierarchy for request");
        cgroup::create_faber_cgroup_hierarchy()?;

        // Create pipe for task results
        eprintln!("[DEBUG] Creating pipe for task results");
        let (results_reader, mut results_writer) = mk_pipe()?;

        eprintln!("[DEBUG] Forking main process");
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                eprintln!("[DEBUG] Parent process: child PID = {}", child.as_raw());
                // Close write end of the pipe in parent
                close_fd(results_writer.into_raw_fd())?;

                // Read and deserialize results to completion first to avoid
                // potential pipe backpressure deadlocks
                let mut results: Option<Vec<TaskResult>> = None;

                eprintln!("[DEBUG] Parent: Starting to read results from pipe");
                // Read with timeout checking
                loop {
                    // Try to read from pipe with a short timeout
                    let mut buffer = Vec::new();
                    let mut temp_reader = match results_reader.try_clone() {
                        Ok(reader) => reader,
                        Err(e) => {
                            eprintln!("[DEBUG] Parent: Failed to clone pipe reader: {:?}", e);
                            continue;
                        }
                    };

                    // Use a short timeout for each read attempt
                    let read_start = Instant::now();
                    let read_timeout_duration = Duration::from_millis(100);

                    loop {
                        if read_start.elapsed() >= read_timeout_duration {
                            break; // Time to check again
                        }

                        // Try to read a small chunk
                        let mut chunk = [0u8; 1024];
                        match temp_reader.read(&mut chunk) {
                            Ok(0) => {
                                eprintln!(
                                    "[DEBUG] Parent: EOF reached, buffer size: {}",
                                    buffer.len()
                                );
                                // EOF reached, try to deserialize
                                if !buffer.is_empty() {
                                    match serde_json::from_slice(&buffer) {
                                        Ok(r) => {
                                            eprintln!(
                                                "[DEBUG] Parent: Successfully deserialized results"
                                            );
                                            results = Some(r);
                                            break;
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "[DEBUG] Parent: Incomplete JSON, continue reading: {:?}",
                                                e
                                            );
                                            // Incomplete JSON, continue reading
                                            continue;
                                        }
                                    }
                                }
                                break;
                            }
                            Ok(n) => {
                                eprintln!("[DEBUG] Parent: Read {} bytes from pipe", n);
                                buffer.extend_from_slice(&chunk[..n]);
                                // Try to deserialize after each chunk
                                match serde_json::from_slice(&buffer) {
                                    Ok(r) => {
                                        eprintln!(
                                            "[DEBUG] Parent: Successfully deserialized results after chunk"
                                        );
                                        results = Some(r);
                                        break;
                                    }
                                    Err(_) => {
                                        // Incomplete JSON, continue reading
                                        continue;
                                    }
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // Would block, continue
                                continue;
                            }
                            Err(e) => {
                                // Other error
                                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                                    eprintln!("[DEBUG] Parent: Unexpected EOF error");
                                    return Err(Error::ProcessManagement {
                                        operation: "read results".to_string(),
                                        pid: child.as_raw(),
                                        details: e.to_string(),
                                    });
                                } else {
                                    eprintln!("[DEBUG] Parent: Read error: {:?}", e);
                                    return Err(Error::ProcessManagement {
                                        operation: "read results".to_string(),
                                        pid: child.as_raw(),
                                        details: format!("Failed to read/deserialize results: {e}"),
                                    });
                                }
                            }
                        }
                    }

                    // If we got results, break out of the main loop
                    if results.is_some() {
                        eprintln!("[DEBUG] Parent: Got results, breaking out of read loop");
                        break;
                    }

                    // Small sleep to prevent busy waiting
                    sleep(Duration::from_millis(10));
                }

                let results = results.unwrap();
                eprintln!("[DEBUG] Parent: Results received, waiting for child to exit");

                let _timeout_killed = crate::utils::wait_for_child_with_timeout(
                    child,
                    Some(5), // Give 5 seconds for cleanup
                )?;

                eprintln!("[DEBUG] Parent: Child exited, cleaning up environment");
                environment::cleanup(&self.host_container_root)?;

                eprintln!("[DEBUG] Parent: Runtime completed successfully");
                Ok(results)
            }
            Ok(ForkResult::Child) => {
                eprintln!("[DEBUG] Child process: Starting execution");
                close_fd(results_reader.into_raw_fd())?;

                eprintln!("[DEBUG] Child: Calling run_in_manager");
                match self.run_in_manager(tasks) {
                    Ok(results) => {
                        eprintln!(
                            "[DEBUG] Child: run_in_manager succeeded, writing {} results",
                            results.len()
                        );
                        serde_json::to_writer(&mut results_writer, &results).map_err(|e| {
                            Error::ProcessManagement {
                                operation: "write results".to_string(),
                                pid: -1,
                                details: format!("Failed to write results: {e}"),
                            }
                        })?;
                        eprintln!("[DEBUG] Child: Results written, exiting");
                        exit(0);
                    }
                    Err(e) => {
                        eprintln!("[DEBUG] Child: run_in_manager failed: {:?}", e);
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
        eprintln!("[DEBUG] run_in_manager: Setting up container environment");
        self.setup_container_environment()?;
        eprintln!(
            "[DEBUG] run_in_manager: Container environment ready, executing {} tasks",
            tasks.len()
        );
        self.execute_tasks(tasks)
    }

    /// Sets up the container environment with namespaces, mounts, and basic filesystem.
    fn setup_container_environment(&self) -> Result<()> {
        eprintln!("[DEBUG] setup_container_environment: Creating container root");
        environment::create_container_root(&self.host_container_root)?;

        let unshare_flags = CloneFlags::CLONE_NEWNS // # mount
            | CloneFlags::CLONE_NEWUTS // # hostname
            | CloneFlags::CLONE_NEWIPC // # ipc
            | CloneFlags::CLONE_NEWNET; // # net

        eprintln!("[DEBUG] setup_container_environment: Unsharing namespaces");
        environment::unshare(unshare_flags)?;
        eprintln!("[DEBUG] setup_container_environment: Binding mounts");
        environment::bind_mounts(&self.host_container_root, &self.mounts)?;
        eprintln!("[DEBUG] setup_container_environment: Pivoting root");
        environment::pivot_root_to(&self.host_container_root)?;
        eprintln!("[DEBUG] setup_container_environment: Creating device files");
        environment::create_dev_devices()?;
        eprintln!("[DEBUG] setup_container_environment: Creating proc");
        environment::create_proc()?;
        eprintln!("[DEBUG] setup_container_environment: Creating sys");
        environment::create_sys()?;
        eprintln!("[DEBUG] setup_container_environment: Creating cgroup");
        environment::create_cgroup()?;
        eprintln!("[DEBUG] setup_container_environment: Creating work directory");
        environment::create_work_dir(&self.work_dir, &self.filesystem_config.workdir_size)?;
        eprintln!("[DEBUG] setup_container_environment: Creating tmp directory");
        environment::create_tmp_dir(&self.work_dir, &self.filesystem_config.tmp_size)?;
        eprintln!("[DEBUG] setup_container_environment: Setting hostname");
        environment::set_container_hostname(&self.hostname)?;

        eprintln!("[DEBUG] setup_container_environment: Container environment setup complete");
        Ok(())
    }

    /// Executes all tasks and returns their results.
    fn execute_tasks(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        eprintln!(
            "[DEBUG] execute_tasks: Starting execution of {} tasks",
            tasks.len()
        );
        let mut results: Vec<TaskResult> = Vec::with_capacity(tasks.len());

        for (i, task) in tasks.iter().enumerate() {
            eprintln!(
                "[DEBUG] execute_tasks: Executing task {}/{}: {:?}",
                i + 1,
                tasks.len(),
                task.cmd
            );

            if let Some(last_task) = results.last() {
                if last_task.exit_code != 0 {
                    eprintln!(
                        "[DEBUG] execute_tasks: Previous task failed, skipping task {}",
                        i + 1
                    );
                    results.push(self.create_skipped_result());
                    continue;
                }
            }

            let task_result = self.execute_single_task(task.clone())?;
            eprintln!(
                "[DEBUG] execute_tasks: Task {}/{} completed with exit code {}",
                i + 1,
                tasks.len(),
                task_result.exit_code
            );

            results.push(task_result);
        }

        eprintln!("[DEBUG] execute_tasks: All {} tasks completed", tasks.len());
        Ok(results)
    }

    /// Executes a single task and returns its result.
    fn execute_single_task(&self, task: Task) -> Result<TaskResult> {
        eprintln!("[DEBUG] execute_single_task: Starting task: {:?}", task.cmd);
        let (task_reader, task_writer) = mk_pipe()?;
        let start_time = Instant::now();

        // Create task-specific cgroup for this task
        eprintln!("[DEBUG] execute_single_task: Creating task-specific cgroup");
        let task_cgroup_path = cgroup::create_task_cgroup(&self.cgroup_config)?;

        eprintln!("[DEBUG] execute_single_task: Forking for task execution");
        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                eprintln!(
                    "[DEBUG] execute_single_task: Parent process: child PID = {}",
                    child.as_raw()
                );
                close_fd(task_writer.into_raw_fd())?;

                let task_result =
                    self.handle_parent_process(child, task_reader, start_time, &task_cgroup_path)?;
                eprintln!("[DEBUG] execute_single_task: Task completed successfully");
                Ok(task_result)
            }
            Ok(ForkResult::Child) => {
                eprintln!("[DEBUG] execute_single_task: Child process starting task execution");
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
        eprintln!("[DEBUG] handle_parent_process: Reading task result from child");
        // Read TaskResult from child
        let mut task_result: TaskResult = match serde_json::de::from_reader(&task_reader) {
            Ok(res) => {
                eprintln!("[DEBUG] handle_parent_process: Successfully read task result");
                res
            }
            Err(e) => {
                eprintln!("[DEBUG] handle_parent_process: Failed to read/deserialize task result: {:?}", e);
                return Err(Error::ProcessManagement {
                    operation: "read task result".to_string(),
                    pid: pid.as_raw(),
                    details: format!("Failed to read/deserialize task result: {e}"),
                });
            }
        };

        // Wait for per-task child to finish
        eprintln!("[DEBUG] handle_parent_process: Waiting for child process {} to exit", pid.as_raw());
        crate::utils::wait_for_child(pid)?;
        eprintln!("[DEBUG] handle_parent_process: Child process {} exited", pid.as_raw());

        // Read task statistics from the task cgroup before cleanup
        eprintln!("[DEBUG] handle_parent_process: Reading task statistics from cgroup");
        let task_stats = match cgroup::read_task_stats(task_cgroup_path) {
            Ok(stats) => {
                eprintln!("[DEBUG] handle_parent_process: Successfully read task statistics");
                stats
            }
            Err(e) => {
                eprintln!("[DEBUG] handle_parent_process: Warning: Failed to read task stats: {:?}", e);
                cgroup::TaskStats::default()
            }
        };

        // Clean up the task cgroup directory
        eprintln!("[DEBUG] handle_parent_process: Cleaning up task cgroup");
        if let Err(e) = cgroup::cleanup_task_cgroup(task_cgroup_path) {
            eprintln!("[DEBUG] handle_parent_process: Warning: Failed to cleanup task cgroup: {:?}", e);
        }

        eprintln!("[DEBUG] handle_parent_process: Populating task result with metrics");
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

        eprintln!("[DEBUG] handle_parent_process: Task result processing complete");
        Ok(task_result)
    }

    /// Handles the child process side of task execution.
    fn handle_child_process(
        &self,
        task: Task,
        mut task_writer: std::io::PipeWriter,
        task_cgroup_path: &str,
    ) -> ! {
        eprintln!("[DEBUG] handle_child_process: Setting up task-specific namespaces");
        // Set up task-specific namespaces
        let flags = CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNS;
        if let Err(e) = environment::unshare(flags) {
            eprintln!("[DEBUG] handle_child_process: unshare failed: {:?}", e);
            let result = self.create_error_result("unshare failed", &e.to_string());
            self.write_result_and_exit(&mut task_writer, &result);
        }

        eprintln!("[DEBUG] handle_child_process: Adding process to task cgroup");
        // Add this child process to the task cgroup
        let current_pid = std::process::id();
        if let Err(e) = cgroup::add_process_to_task_cgroup(task_cgroup_path, current_pid) {
            eprintln!(
                "[DEBUG] handle_child_process: add process to cgroup failed: {:?}",
                e
            );
            let result = self.create_error_result("add process to cgroup failed", &e.to_string());
            self.write_result_and_exit(&mut task_writer, &result);
        }

        // Write files if specified
        if let Some(files) = &task.files {
            eprintln!(
                "[DEBUG] handle_child_process: Writing {} files",
                files.len()
            );
            if let Err(e) = environment::write_files(&self.work_dir, files) {
                eprintln!("[DEBUG] handle_child_process: write files failed: {:?}", e);
                let result = self.create_error_result("write files failed", &e.to_string());
                self.write_result_and_exit(&mut task_writer, &result);
            }
        }

        eprintln!(
            "[DEBUG] handle_child_process: Preparing and spawning command: {:?}",
            task.cmd
        );
        // Prepare and spawn the command
        let child = match self.prepare_and_spawn_command(&task, &mut task_writer) {
            Ok(child) => child,
            Err(_) => exit(0), // Error already written to pipe
        };

        eprintln!("[DEBUG] handle_child_process: Command spawned, waiting for completion");
        // Wait for command to finish and get result
        let output = match child.wait_with_output() {
            Ok(o) => {
                eprintln!(
                    "[DEBUG] handle_child_process: Command completed, exit code: {}",
                    o.status
                );
                o
            }
            Err(e) => {
                eprintln!("[DEBUG] handle_child_process: wait failed: {:?}", e);
                let result = self.create_error_result("wait failed", &e.to_string());
                self.write_result_and_exit(&mut task_writer, &result);
            }
        };

        eprintln!("[DEBUG] handle_child_process: Parsing command output");
        // Parse the command output
        let result = TaskResult::from(output);

        eprintln!("[DEBUG] handle_child_process: Writing result to pipe and exiting");
        // Write the result to the pipe and exit
        self.write_result_and_exit(&mut task_writer, &result);
    }

    /// Prepares and spawns a command for execution.
    fn prepare_and_spawn_command(
        &self,
        task: &Task,
        task_writer: &mut std::io::PipeWriter,
    ) -> Result<std::process::Child> {
        eprintln!(
            "[DEBUG] prepare_and_spawn_command: Building command: {:?}",
            task.cmd
        );
        let mut cmd = Command::new(&task.cmd);
        cmd.current_dir(&self.work_dir);

        if let Some(args) = &task.args {
            eprintln!(
                "[DEBUG] prepare_and_spawn_command: Adding {} arguments",
                args.len()
            );
            cmd.args(args);
        }

        eprintln!("[DEBUG] prepare_and_spawn_command: Clearing environment");
        cmd.env_clear();

        if let Some(env) = &task.env {
            eprintln!(
                "[DEBUG] prepare_and_spawn_command: Setting {} environment variables",
                env.len()
            );
            cmd.envs(env);
        }

        let has_path = cmd.get_envs().any(|(key, _)| key == "PATH");
        if !has_path {
            eprintln!("[DEBUG] prepare_and_spawn_command: Setting default PATH");
            cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
        }

        if task.stdin.is_some() {
            eprintln!("[DEBUG] prepare_and_spawn_command: Setting up stdin pipe");
            cmd.stdin(std::process::Stdio::piped());
        } else {
            eprintln!("[DEBUG] prepare_and_spawn_command: Setting stdin to null");
            cmd.stdin(std::process::Stdio::null());
        }

        eprintln!("[DEBUG] prepare_and_spawn_command: Setting up stdout/stderr pipes");
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        eprintln!("[DEBUG] prepare_and_spawn_command: Dropping privileges to nobody user");
        // Drop privileges to nobody user for security before spawning the command
        if let Err(e) = environment::drop_privileges_to_nobody() {
            eprintln!(
                "[DEBUG] prepare_and_spawn_command: failed to drop privileges: {:?}",
                e
            );
            let result = self.create_error_result("failed to drop privileges", &e.to_string());
            self.write_result_and_exit(task_writer, &result);
        }

        eprintln!("[DEBUG] prepare_and_spawn_command: Spawning command");
        cmd.spawn().map_err(|e| {
            eprintln!("[DEBUG] prepare_and_spawn_command: spawn failed: {:?}", e);
            let result = self.create_error_result("spawn failed", &e.to_string());
            self.write_result_and_exit(task_writer, &result);
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
