use nix::sched::CloneFlags;
use nix::unistd::{ForkResult, fork};

use rand::Rng;
use rand::distr::Alphanumeric;

use std::io::{PipeWriter, Write};
use std::os::fd::IntoRawFd;
use std::path::PathBuf;
use std::process::{Command, exit};
use std::time::{Duration, Instant};

use crate::TaskResult;
use crate::builder::RuntimeBuilder;
use crate::cgroup;
use crate::environment;
use crate::prelude::*;
use crate::types::{CgroupConfig, FilesystemConfig, Mount, RuntimeLimits, Task};
use crate::utils::{close_fd, mk_pipe, wait_for_child};

/// High-level entry point for preparing an isolated environment and running tasks.
#[derive(Debug)]
pub struct Runtime {
    pub(crate) host_container_root: PathBuf,
    pub(crate) hostname: String,
    pub(crate) mounts: Vec<Mount>,
    pub(crate) work_dir: PathBuf,
    pub(crate) filesystem_config: FilesystemConfig,
    pub(crate) cgroup: CgroupConfig,
    pub(crate) runtime_limits: RuntimeLimits,
}

impl Runtime {
    /// Get a builder to configure and construct a [`Runtime`].
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn run(self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Validate tasks
        self.validate_tasks(&tasks)?;

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

                wait_for_child(child)?;

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
        environment::create_container_root(&self.host_container_root)?;

        let unshare_flags = CloneFlags::CLONE_NEWNS // # mount
            | CloneFlags::CLONE_NEWUTS // # hostname
            | CloneFlags::CLONE_NEWIPC // # ipc
            | CloneFlags::CLONE_NEWNET; // # net

        environment::unshare(unshare_flags)?;
        environment::bind_mounts(&self.host_container_root, &self.mounts)?;
        environment::pivot_root_to(&self.host_container_root)?;
        environment::create_dev_devices()?;
        environment::create_proc()?; // #Not used for now
        environment::create_sys()?;
        environment::create_cgroup()?;
        environment::create_work_dir(&self.work_dir, &self.filesystem_config.workdir_size)?;
        environment::create_tmp_dir(&self.work_dir, &self.filesystem_config.tmp_size)?;
        environment::set_container_hostname(&self.hostname)?;

        let mut results = Vec::with_capacity(tasks.len());
        let mut skip_rest = false;

        for t in tasks.into_iter() {
            // If a previous task failed, mark remaining tasks as skipped without executing them
            if skip_rest {
                results.push(TaskResult {
                    stdout: String::new(),
                    stderr: "skipped: previous task failed".to_string(),
                    exit_code: -1,
                    execution_time_ms: None,
                    cpu_usage_usec: None,
                    cpu_user_usec: None,
                    cpu_system_usec: None,
                    memory_peak_bytes: None,
                });
                continue;
            }

            // Create a pipe for this task's result
            let (task_reader, mut task_writer) = mk_pipe()?;

            // Track wall time
            let start_time = Instant::now();

            match unsafe { fork() } {
                Ok(ForkResult::Parent { child, .. }) => {
                    // Parent side: set up and attach cgroup if enabled
                    let mut task_cg: Option<PathBuf> = None;
                    if self.cgroup.enabled {
                        let base = PathBuf::from("/sys/fs/cgroup/faber");
                        cgroup::create_cgroup_at(&base)?;
                        cgroup::enable_subtree_controllers_at(&base, &["pids", "memory", "cpu"])?;
                        let task_id: String = rand::rng()
                            .sample_iter(&Alphanumeric)
                            .take(16)
                            .map(char::from)
                            .collect();
                        let cg = base.join(format!("task-{task_id}"));
                        cgroup::create_cgroup_at(&cg)?;
                        cgroup::set_limits(&cg, &self.cgroup)?;
                        cgroup::add_pid(&cg, child.as_raw())?;

                        cgroup::debug(&cg.join("memory.max"));
                        task_cg = Some(cg);
                    }

                    // Close writer end
                    close_fd(task_writer.into_raw_fd())?;

                    // Read TaskResult from child
                    let mut task_result: TaskResult =
                        match serde_json::de::from_reader(&task_reader) {
                            Ok(res) => res,
                            Err(e) => {
                                return Err(Error::ProcessManagement {
                                    operation: "read task result".to_string(),
                                    pid: child.as_raw(),
                                    details: format!("Failed to read/deserialize task result: {e}"),
                                });
                            }
                        };

                    // Wait for per-task child to finish
                    wait_for_child(child)?;

                    // Populate metrics
                    task_result.execution_time_ms = Some(start_time.elapsed().as_millis() as u64);
                    if let Some(cg) = task_cg {
                        // cpu.stat
                        let cpu_stat_path = cg.join("cpu.stat");
                        if let Ok(contents) = std::fs::read_to_string(&cpu_stat_path) {
                            for line in contents.lines() {
                                let mut parts = line.split_whitespace();
                                if let (Some(key), Some(val)) = (parts.next(), parts.next()) {
                                    if let Ok(num) = val.parse::<u64>() {
                                        match key {
                                            "usage_usec" => task_result.cpu_usage_usec = Some(num),
                                            "user_usec" => task_result.cpu_user_usec = Some(num),
                                            "system_usec" => {
                                                task_result.cpu_system_usec = Some(num)
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                        // memory.peak
                        let mem_peak_path = cg.join("memory.peak");
                        if let Ok(s) = std::fs::read_to_string(&mem_peak_path) {
                            if let Ok(num) = s.trim().parse::<u64>() {
                                task_result.memory_peak_bytes = Some(num);
                            }
                        }

                        cgroup::remove_cgroup(&cg)?;
                    }

                    // If non-zero exit, mark the rest as skipped
                    if task_result.exit_code != 0 {
                        skip_rest = true;
                    }

                    results.push(task_result);
                }
                Ok(ForkResult::Child) => {
                    close_fd(task_reader.into_raw_fd())?;

                    let flags = CloneFlags::CLONE_NEWPID | CloneFlags::CLONE_NEWNS;
                    if let Err(e) = environment::unshare(flags) {
                        let result = TaskResult {
                            stdout: String::new(),
                            stderr: format!("unshare failed: {e}"),
                            exit_code: -1,
                            execution_time_ms: None,
                            cpu_usage_usec: None,
                            cpu_user_usec: None,
                            cpu_system_usec: None,
                            memory_peak_bytes: None,
                        };
                        let _ = serde_json::to_writer(&mut task_writer, &result);
                        let _ = task_writer.flush();
                        exit(0);
                    }

                    if let Err(e) = environment::mask_mounts(&["/proc", "/sys"]) {
                        let result = TaskResult {
                            stdout: String::new(),
                            stderr: format!("mask mounts failed: {e}"),
                            exit_code: -1,
                            execution_time_ms: None,
                            cpu_usage_usec: None,
                            cpu_user_usec: None,
                            cpu_system_usec: None,
                            memory_peak_bytes: None,
                        };
                        let _ = serde_json::to_writer(&mut task_writer, &result);
                        let _ = task_writer.flush();
                        exit(0);
                    }

                    if let Some(files) = &t.files {
                        if let Err(e) = environment::write_files(&self.work_dir, files) {
                            let result = TaskResult {
                                stdout: String::new(),
                                stderr: format!("write files failed: {e}"),
                                exit_code: -1,
                                execution_time_ms: None,
                                cpu_usage_usec: None,
                                cpu_user_usec: None,
                                cpu_system_usec: None,
                                memory_peak_bytes: None,
                            };
                            let _ = serde_json::to_writer(&mut task_writer, &result);
                            let _ = task_writer.flush();
                            exit(0);
                        }
                    }

                    // list all files
                    let files = std::fs::read_dir(&self.work_dir).map_err(|e| Error::Io {
                        operation: "read workdir".to_string(),
                        path: self.work_dir.to_string_lossy().to_string(),
                        details: format!("Failed to read workdir: {e}"),
                    })?;
                    for file in files {
                        println!("File: {:?}", file.unwrap().path());
                    }

                    let mut cmd = Command::new(&t.cmd);
                    cmd.current_dir(&self.work_dir);

                    if let Some(args) = &t.args {
                        cmd.args(args);
                    }

                    cmd.env_clear();

                    if let Some(env) = &t.env {
                        cmd.envs(env);
                    }

                    let has_path = cmd.get_envs().any(|(key, _)| key == "PATH");
                    if !has_path {
                        cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
                    }

                    if t.stdin.is_some() {
                        cmd.stdin(std::process::Stdio::piped());
                    } else {
                        cmd.stdin(std::process::Stdio::null());
                    }

                    cmd.stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped());

                    let mut child = match cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            let result = TaskResult {
                                stdout: String::new(),
                                stderr: format!("spawn failed: {e}"),
                                exit_code: -1,
                                execution_time_ms: None,
                                cpu_usage_usec: None,
                                cpu_user_usec: None,
                                cpu_system_usec: None,
                                memory_peak_bytes: None,
                            };
                            let _ = serde_json::to_writer(&mut task_writer, &result);
                            let _ = task_writer.flush();
                            exit(0);
                        }
                    };

                    let output = if let Some(secs) = self.runtime_limits.kill_timeout_seconds {
                        let deadline = Instant::now() + Duration::from_secs(secs);
                        loop {
                            if Instant::now() >= deadline {
                                let _ = child.kill();
                                break match child.wait_with_output() {
                                    Ok(o) => o,
                                    Err(e) => {
                                        let result = TaskResult {
                                            stdout: String::new(),
                                            stderr: format!("wait failed after kill: {e}"),
                                            exit_code: -1,
                                            execution_time_ms: None,
                                            cpu_usage_usec: None,
                                            cpu_user_usec: None,
                                            cpu_system_usec: None,
                                            memory_peak_bytes: None,
                                        };
                                        let _ = serde_json::to_writer(&mut task_writer, &result);
                                        let _ = task_writer.flush();
                                        exit(0);
                                    }
                                };
                            }
                            match child.try_wait() {
                                Ok(Some(_status)) => {
                                    break match child.wait_with_output() {
                                        Ok(o) => o,
                                        Err(e) => {
                                            let result = TaskResult {
                                                stdout: String::new(),
                                                stderr: format!("wait failed: {e}"),
                                                exit_code: -1,
                                                execution_time_ms: None,
                                                cpu_usage_usec: None,
                                                cpu_user_usec: None,
                                                cpu_system_usec: None,
                                                memory_peak_bytes: None,
                                            };
                                            let _ =
                                                serde_json::to_writer(&mut task_writer, &result);
                                            let _ = task_writer.flush();
                                            exit(0);
                                        }
                                    };
                                }
                                Ok(None) => {
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                                Err(e) => {
                                    let result = TaskResult {
                                        stdout: String::new(),
                                        stderr: format!("try_wait failed: {e}"),
                                        exit_code: -1,
                                        execution_time_ms: None,
                                        cpu_usage_usec: None,
                                        cpu_user_usec: None,
                                        cpu_system_usec: None,
                                        memory_peak_bytes: None,
                                    };
                                    let _ = serde_json::to_writer(&mut task_writer, &result);
                                    let _ = task_writer.flush();
                                    exit(0);
                                }
                            }
                        }
                    } else {
                        match child.wait_with_output() {
                            Ok(o) => o,
                            Err(e) => {
                                let result = TaskResult {
                                    stdout: String::new(),
                                    stderr: format!("wait failed: {e}"),
                                    exit_code: -1,
                                    execution_time_ms: None,
                                    cpu_usage_usec: None,
                                    cpu_user_usec: None,
                                    cpu_system_usec: None,
                                    memory_peak_bytes: None,
                                };
                                let _ = serde_json::to_writer(&mut task_writer, &result);
                                let _ = task_writer.flush();
                                exit(0);
                            }
                        }
                    };

                    let result = TaskResult::from(output);

                    if serde_json::to_writer(&mut task_writer, &result).is_err() {
                        // If writing fails, just exit; parent will see read error, but we tried
                    }
                    let _ = task_writer.flush();

                    exit(0);
                }
                Err(e) => {
                    return Err(Error::ProcessManagement {
                        operation: "fork per-task".to_string(),
                        pid: -1,
                        details: format!("Fork failed in task loop: {e:?}"),
                    });
                }
            }
        }

        Ok(results)
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
