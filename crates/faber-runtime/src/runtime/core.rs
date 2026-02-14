use std::{
    ffi::CString,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd, IntoRawFd},
    path::PathBuf,
    process::exit,
    time::Duration,
};

use caps::CapSet;
use nix::{
    libc,
    sched::{unshare, CloneFlags},
    sys::wait::{waitpid, WaitPidFlag, WaitStatus},
    unistd::{execvpe, fork, pipe, setgid, setuid, ForkResult, Pid},
};

use crate::{
    cgroup::Cgroup,
    container::Container,
    prelude::*,
    result::{ExecutionStepResult, RuntimeResult, TaskResult, TaskResultStats},
    task::{ExecutionStep, Task, TaskGroup},
    utils::{close_fd, mk_pipe},
};

pub struct Runtime {
    pub(crate) task_group: TaskGroup,
    pub(crate) container: Container,
    pub(crate) cgroup: Cgroup,
    pub(crate) timeout: Duration,
}

impl Runtime {
    pub fn execute(&self) -> Result<RuntimeResult> {
        Cgroup::ensure_faber_cgroup_hierarchy()?;

        let (reader, writer) = mk_pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                close_fd(reader.into_raw_fd())?;

                let runtime_result = self.execution_child();
                let _ = serde_json::to_writer(writer, &runtime_result);
                exit(0);
            }
            Ok(ForkResult::Parent { child }) => {
                close_fd(writer.into_raw_fd())?;
                waitpid(child, None).map_err(|e| FaberError::WaitPid { e })?;

                let runtime_result: RuntimeResult =
                    serde_json::from_reader(reader).map_err(|e| FaberError::ParseResult {
                        e,
                        details: "Failed to parse results from child process".to_string(),
                    })?;

                if let Err(e) = self.container.cleanup() {
                    eprintln!("Failed to cleanup container: {}", e);
                }

                Ok(runtime_result)
            }
            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    fn execution_child(&self) -> RuntimeResult {
        if let Err(e) = self.container.setup() {
            return RuntimeResult::ContainerSetupFailed {
                error: format!("Container setup failed: {}", e),
            };
        }

        // Fork a dedicated "init" process to keep the PID namespace alive.
        // container.setup() calls unshare(CLONE_NEWPID), so the first child we
        // fork becomes PID 1 in the new namespace. If PID 1 exits, the kernel
        // destroys the namespace and all subsequent forks fail with ENOMEM.
        // This init process stays alive for the duration of task execution,
        // allowing task children to get PID 2, 3, etc.
        let init_pid = match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                // PID 1 in the new namespace — just sleep until killed
                loop {
                    std::thread::sleep(Duration::from_secs(3600));
                }
            }
            Ok(ForkResult::Parent { child }) => child,
            Err(e) => {
                return RuntimeResult::ContainerSetupFailed {
                    error: format!("Failed to fork namespace init process: {}", e),
                };
            }
        };

        let mut results = Vec::with_capacity(self.task_group.len());

        for step in &self.task_group {
            let result = match step {
                ExecutionStep::Single(task) => self.execute_single(task.clone()),
                ExecutionStep::Parallel(tasks) => self.execute_parallel(tasks.clone()),
            };
            results.push(result);
        }

        // Tear down the namespace init process
        let _ = nix::sys::signal::kill(init_pid, nix::sys::signal::Signal::SIGKILL);
        let _ = waitpid(init_pid, None);

        RuntimeResult::Success(results)
    }

    fn execute_single(&self, task: Task) -> ExecutionStepResult {
        match Self::execute_single_task(task, &self.cgroup, self.timeout) {
            Ok(task_result) => ExecutionStepResult::Single(task_result),
            Err(e) => ExecutionStepResult::Single(TaskResult::Failed {
                error: format!("Task execution failed: {}", e),
                stats: TaskResultStats::default(),
            }),
        }
    }

    fn execute_parallel(&self, tasks: Vec<Task>) -> ExecutionStepResult {
        // Cannot use std::thread::spawn after unshare(CLONE_NEWPID) because
        // the kernel rejects CLONE_THREAD when pid_ns_for_children differs
        // from the active PID namespace (EINVAL). Use fork + pipes instead.
        let mut children: Vec<(Pid, std::io::PipeReader)> = Vec::with_capacity(tasks.len());

        for task in tasks {
            let pipe = match mk_pipe() {
                Ok(p) => p,
                Err(e) => {
                    return ExecutionStepResult::Parallel(vec![TaskResult::Failed {
                        error: format!("Failed to create pipe for parallel task: {}", e),
                        stats: TaskResultStats::default(),
                    }]);
                }
            };
            let (reader, writer) = pipe;

            match unsafe { fork() } {
                Ok(ForkResult::Child) => {
                    drop(reader);
                    let result =
                        match Self::execute_single_task(task, &self.cgroup, self.timeout) {
                            Ok(task_result) => task_result,
                            Err(e) => TaskResult::Failed {
                                error: format!("Task execution failed: {}", e),
                                stats: TaskResultStats::default(),
                            },
                        };
                    let _ = serde_json::to_writer(writer, &result);
                    exit(0);
                }
                Ok(ForkResult::Parent { child }) => {
                    drop(writer);
                    children.push((child, reader));
                }
                Err(e) => {
                    return ExecutionStepResult::Parallel(vec![TaskResult::Failed {
                        error: format!("Failed to fork parallel task: {}", e),
                        stats: TaskResultStats::default(),
                    }]);
                }
            }
        }

        // Wait for all parallel children and collect results
        let mut task_results = Vec::with_capacity(children.len());
        for (child, reader) in children {
            let _ = waitpid(child, None);
            let result: TaskResult = serde_json::from_reader(reader).unwrap_or(TaskResult::Failed {
                error: "Failed to read result from parallel task".to_string(),
                stats: TaskResultStats::default(),
            });
            task_results.push(result);
        }

        ExecutionStepResult::Parallel(task_results)
    }

    fn execute_single_task(
        task: Task,
        cgroup: &Cgroup,
        timeout: std::time::Duration,
    ) -> Result<TaskResult> {
        use std::time::Instant;

        let start_time = Instant::now();

        // Create task cgroup before fork
        let task_cgroup = cgroup.create_task_cgroup()?;

        // Write files before fork (in parent's namespace context)
        for (file_path, file_content) in task.files.clone().unwrap_or_default() {
            let file_path = PathBuf::from(file_path);
            std::fs::write(file_path, file_content).map_err(|e| FaberError::WriteFile {
                e,
                details: "Failed to write file".to_string(),
            })?;
        }

        // Create pipes for stdout, stderr, stdin
        let (stdout_read, stdout_write) = pipe().map_err(|e| FaberError::MkPipe {
            e: std::io::Error::from_raw_os_error(e as i32),
            details: "Failed to create stdout pipe".to_string(),
        })?;
        let (stderr_read, stderr_write) = pipe().map_err(|e| FaberError::MkPipe {
            e: std::io::Error::from_raw_os_error(e as i32),
            details: "Failed to create stderr pipe".to_string(),
        })?;
        let (stdin_read, stdin_write) = pipe().map_err(|e| FaberError::MkPipe {
            e: std::io::Error::from_raw_os_error(e as i32),
            details: "Failed to create stdin pipe".to_string(),
        })?;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                // FIRST: Add self to cgroup BEFORE any other work
                // This ensures resource limits apply from the start
                let my_pid = std::process::id();
                if let Err(e) = task_cgroup.add_process(my_pid) {
                    eprintln!("Failed to add process to cgroup: {}", e);
                    exit(127);
                }

                // Close read ends of pipes in child
                drop(stdout_read);
                drop(stderr_read);
                drop(stdin_write);

                // Redirect stdout/stderr/stdin using libc dup2
                unsafe {
                    libc::dup2(stdout_write.as_raw_fd(), libc::STDOUT_FILENO);
                    libc::dup2(stderr_write.as_raw_fd(), libc::STDERR_FILENO);
                    libc::dup2(stdin_read.as_raw_fd(), libc::STDIN_FILENO);
                }

                // Close original fds after dup2
                drop(stdout_write);
                drop(stderr_write);
                drop(stdin_read);

                // Apply security restrictions
                if let Err(e) = Self::child_setup_security() {
                    eprintln!("Security setup failed: {}", e);
                    exit(126);
                }

                // Build environment
                let mut env_vars: Vec<(CString, CString)> = Vec::new();
                let mut has_path = false;

                for (key, value) in task.env.unwrap_or_default() {
                    if key == "PATH" {
                        has_path = true;
                    }
                    if let (Ok(k), Ok(v)) = (CString::new(key.clone()), CString::new(value)) {
                        env_vars.push((k, v));
                    }
                }

                if !has_path {
                    if let (Ok(k), Ok(v)) = (
                        CString::new("PATH"),
                        CString::new(
                            "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                        ),
                    ) {
                        env_vars.push((k, v));
                    }
                }

                // Build args
                let cmd_cstr = match CString::new(task.cmd.clone()) {
                    Ok(c) => c,
                    Err(_) => exit(127),
                };

                let mut args_cstr: Vec<CString> = vec![cmd_cstr.clone()];
                if let Some(args) = task.args {
                    for arg in args {
                        if let Ok(a) = CString::new(arg) {
                            args_cstr.push(a);
                        }
                    }
                }

                // Format env as "KEY=VALUE"
                let env_cstr: Vec<CString> = env_vars
                    .into_iter()
                    .filter_map(|(k, v)| {
                        let s = format!("{}={}", k.to_string_lossy(), v.to_string_lossy());
                        CString::new(s).ok()
                    })
                    .collect();

                // Execute
                let _ = execvpe(&cmd_cstr, &args_cstr, &env_cstr);

                // If exec fails, exit with error
                exit(127);
            }
            Ok(ForkResult::Parent { child }) => {
                // Close write ends of pipes in parent
                drop(stdout_write);
                drop(stderr_write);
                drop(stdin_read);

                // Write stdin if provided
                if let Some(stdin_data) = task.stdin {
                    let mut stdin_file =
                        unsafe { std::fs::File::from_raw_fd(stdin_write.into_raw_fd()) };
                    if let Err(e) = stdin_file.write_all(stdin_data.as_bytes()) {
                        eprintln!("Warning: Failed to write stdin: {}", e);
                    }
                    // stdin_file is dropped here, closing the write end
                } else {
                    drop(stdin_write);
                }

                // Wait with timeout
                let exit_code = Self::wait_for_child_with_timeout(child, timeout)?;

                // Read stdout and stderr
                let mut stdout_buf = String::new();
                let mut stderr_buf = String::new();

                let mut stdout_file =
                    unsafe { std::fs::File::from_raw_fd(stdout_read.into_raw_fd()) };
                let mut stderr_file =
                    unsafe { std::fs::File::from_raw_fd(stderr_read.into_raw_fd()) };

                if let Err(e) = stdout_file.read_to_string(&mut stdout_buf) {
                    eprintln!("Warning: Failed to read stdout: {}", e);
                }
                if let Err(e) = stderr_file.read_to_string(&mut stderr_buf) {
                    eprintln!("Warning: Failed to read stderr: {}", e);
                }

                // Measure resources
                let task_stats = match task_cgroup.measure_resources() {
                    Ok(stats) => stats,
                    Err(e) => {
                        eprintln!("Warning: Failed to measure resources: {}", e);
                        Default::default()
                    }
                };

                // Cleanup cgroup
                if let Err(e) = task_cgroup.cleanup() {
                    eprintln!("Warning: Failed to cleanup task cgroup: {}", e);
                }

                let stats = TaskResultStats {
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    memory_peak_bytes: task_stats.memory_peak_bytes,
                    cpu_usage_usec: task_stats.cpu_usage_usec,
                    pids_peak: task_stats.pids_max,
                };

                Ok(TaskResult::Completed {
                    stdout: stdout_buf,
                    stderr: stderr_buf,
                    exit_code,
                    stats,
                })
            }
            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    /// Set up security restrictions in child process before exec
    fn child_setup_security() -> std::io::Result<()> {
        let unshare_flags = CloneFlags::CLONE_NEWNS;

        unshare(unshare_flags).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // mask_paths unmounts and masks proc/sys with tmpfs for security
        Container::mask_paths()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // Mount a fresh proc filesystem in the child's PID namespace
        // This is critical: the child is PID 1 in the new PID namespace,
        // so mounting proc here will show only the namespace's processes
        Self::mount_proc_in_pid_namespace()?;

        // Mount sys from oldroot (sysfs doesn't have PID-specific info)
        Self::mount_sys()?;

        setgid(65534.into()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        setuid(65534.into()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        Self::drop_capabilities()?;
        Self::apply_seccomp_filter()?;

        Ok(())
    }

    /// Mount proc filesystem in the child's PID namespace
    /// This MUST be called after the child enters the PID namespace
    fn mount_proc_in_pid_namespace() -> std::io::Result<()> {
        use nix::mount::{mount, MsFlags};

        // Create /proc directory
        std::fs::create_dir_all("/proc").ok();

        // Mount a fresh proc filesystem
        // This will show only the processes in the current PID namespace
        // because we're calling this from the child that is PID 1 in the new namespace
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;
        match mount(
            Some("proc"),
            "/proc",
            Some("proc"),
            proc_flags,
            None::<&str>,
        ) {
            Ok(_) => {}
            Err(_) => {
                // Fresh procfs mount may fail in rootless Docker (EPERM).
                // Fallback: bind mount from oldroot (less secure but functional)
                mount(
                    Some("/oldroot/proc"),
                    "/proc",
                    None::<&str>,
                    MsFlags::MS_BIND,
                    None::<&str>,
                )
                .ok();
            }
        }

        Ok(())
    }

    /// Mount sysfs in the new mount namespace
    fn mount_sys() -> std::io::Result<()> {
        use nix::mount::{mount, MsFlags};

        std::fs::create_dir_all("/sys").ok();

        // Try to mount new sysfs first
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;
        if mount(None::<&str>, "/sys", Some("sysfs"), sys_flags, None::<&str>).is_ok() {
            return Ok(());
        }

        // Fallback: bind mount from oldroot
        mount(
            Some("/oldroot/sys"),
            "/sys",
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .ok();

        Ok(())
    }

    /// Wait for child process with timeout using WNOHANG polling
    fn wait_for_child_with_timeout(child: Pid, timeout: Duration) -> Result<i32> {
        use std::thread;
        use std::time::Instant;

        let start_time = Instant::now();

        loop {
            match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::Exited(_, code)) => return Ok(code),
                Ok(WaitStatus::Signaled(_, signal, _)) => {
                    // Process was killed by signal, return 128 + signal number
                    return Ok(128 + signal as i32);
                }
                Ok(WaitStatus::StillAlive) => {
                    if start_time.elapsed() > timeout {
                        // Kill the child process
                        let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
                        // Reap the zombie
                        let _ = waitpid(child, None);

                        return Err(FaberError::TaskTimeout {
                            timeout_duration: timeout,
                            details: format!(
                                "Task exceeded timeout of {} seconds",
                                timeout.as_secs()
                            ),
                        });
                    }
                    // Short sleep for efficient polling
                    thread::sleep(Duration::from_millis(10));
                }
                Ok(_) => {
                    // Other wait statuses (Stopped, Continued, etc.) - continue waiting
                    thread::sleep(Duration::from_millis(10));
                }
                Err(nix::errno::Errno::ECHILD) => {
                    // Child already reaped
                    return Ok(-1);
                }
                Err(e) => {
                    return Err(FaberError::WaitPid { e });
                }
            }
        }
    }

    fn drop_capabilities() -> std::io::Result<()> {
        caps::clear(None, CapSet::Effective).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Failed to clear effective capabilities: {}", e),
            )
        })?;

        caps::clear(None, CapSet::Permitted).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Failed to clear permitted capabilities: {}", e),
            )
        })?;

        caps::clear(None, CapSet::Inheritable).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Failed to clear inheritable capabilities: {}", e),
            )
        })?;

        Ok(())
    }

    fn apply_seccomp_filter() -> std::io::Result<()> {
        Ok(())
    }
}
