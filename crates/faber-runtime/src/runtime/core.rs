use std::{
    ffi::CString,
    fs::OpenOptions,
    io::{PipeReader, PipeWriter, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::{Component, Path},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use caps::CapSet;
use nix::{
    libc,
    sched::{CloneFlags, unshare},
    sys::wait::{WaitPidFlag, WaitStatus, waitpid},
    unistd::{ForkResult, Pid, chdir, execvpe, fork, pipe, setgid, setgroups, setpgid, setuid},
};

#[cfg(target_env = "gnu")]
type RlimitResource = libc::__rlimit_resource_t;
#[cfg(target_env = "musl")]
type RlimitResource = libc::c_int;

use crate::{
    CancellationToken,
    cgroup::{Cgroup, request::RequestCgroup, task::TaskCgroup},
    container::Container,
    prelude::*,
    result::{ExecutionStepResult, RuntimeResult, TaskOutcome, TaskResult, TaskResultStats},
    task::{ExecutionStep, SandboxProfile, Task, TaskGroup},
    utils::{close_fd, mk_pipe},
};

/// Set once by [`Runtime::shutdown`]; every running execution observes it and
/// tears itself down, and no new execution starts.
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

pub struct Runtime {
    pub(crate) task_group: TaskGroup,
    pub(crate) container: Container,
    pub(crate) cgroup: Cgroup,
    pub(crate) timeout: Duration,
    pub(crate) cpu_time_limit: Duration,
    pub(crate) output_limit: usize,
    pub(crate) request_output_limit: usize,
    pub(crate) overall_timeout: Duration,
    pub(crate) cancellation: CancellationToken,
}

/// Per-task limits for one step, derived from the runtime limits and what
/// remains of the request's overall budgets.
#[derive(Clone, Copy)]
struct StepLimits {
    timeout: Duration,
    output: OutputLimits,
}

/// Output bytes one task may keep: per stream, and across both streams.
#[derive(Clone, Copy)]
struct OutputLimits {
    per_stream: usize,
    total: usize,
}

/// Grace the controller allows beyond the overall deadline before it kills
/// the execution child itself. The child enforces the deadline between and
/// within steps, so this only fires if the child stops making progress.
const OVERALL_DEADLINE_BACKSTOP_GRACE: Duration = Duration::from_secs(2);

struct CollectedOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
    stdout_truncated: bool,
    stderr_truncated: bool,
    termination_signal: Option<i32>,
    timed_out: bool,
    output_terminated: bool,
}

impl Runtime {
    pub fn initialize() -> Result<()> {
        Cgroup::ensure_faber_cgroup_hierarchy()
    }

    /// Configure the aggregate Faber cgroup for `task_slots` concurrently
    /// running tasks with the default container workspace sizes.
    pub fn configure_service_limits(
        per_task_memory: &str,
        per_task_pids: u32,
        task_slots: usize,
    ) -> Result<()> {
        let workspace_allowance =
            Container::default()
                .workspace_allowance()
                .ok_or_else(|| FaberError::Generic {
                    message: "Default container workspace sizes are not byte sizes".to_string(),
                })?;
        Cgroup::configure_service_limits(
            per_task_memory,
            per_task_pids,
            task_slots,
            workspace_allowance,
        )
    }

    /// The most tasks any step of this runtime runs at once.
    pub fn widest_step(task_group: &[ExecutionStep]) -> usize {
        task_group
            .iter()
            .map(|step| match step {
                ExecutionStep::Single(_) => 1,
                ExecutionStep::Parallel(tasks) => tasks.len(),
            })
            .max()
            .unwrap_or(1)
    }

    /// Stop the runtime for service shutdown. New executions are refused;
    /// running ones kill their execution child, PID namespace and request
    /// cgroup within one poll interval and return [`FaberError::ShuttingDown`].
    /// Every request cgroup is also killed here directly.
    pub fn shutdown() -> Result<()> {
        SHUTTING_DOWN.store(true, Ordering::SeqCst);
        Cgroup::kill_active_tasks()
    }

    pub fn is_shutting_down() -> bool {
        SHUTTING_DOWN.load(Ordering::SeqCst)
    }

    pub fn execute(&self) -> Result<RuntimeResult> {
        if Self::is_shutting_down() {
            return Err(FaberError::ShuttingDown);
        }
        if self.cancellation.is_cancelled() {
            return Err(FaberError::Cancelled);
        }
        Cgroup::ensure_faber_cgroup_hierarchy()?;
        let faber_cgroup_path = Cgroup::get_faber_cgroup_path()?;
        let request_cgroup = self.cgroup.create_request_cgroup(
            &faber_cgroup_path,
            Self::widest_step(&self.task_group),
            self.container.workspace_allowance(),
        )?;

        let (reader, writer) = mk_pipe()?;
        let deadline = std::time::Instant::now() + self.overall_timeout;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                drop(reader);
                let _ = setpgid(Pid::from_raw(0), Pid::from_raw(0));

                let runtime_result = self.execution_child(request_cgroup.path(), deadline);
                Self::write_child_result(writer, &runtime_result);
                Self::child_exit(0);
            }
            Ok(ForkResult::Parent { child }) => {
                close_fd(writer.into_raw_fd())?;
                let _ = setpgid(child, child);

                let runtime_result = self.read_runtime_result(
                    child,
                    reader,
                    &request_cgroup,
                    deadline + OVERALL_DEADLINE_BACKSTOP_GRACE,
                );

                if let Err(error) = self.container.cleanup() {
                    tracing::error!(%error, "failed to cleanup container");
                }
                if let Err(error) = request_cgroup.cleanup() {
                    tracing::error!(%error, "failed to cleanup request cgroup");
                }

                runtime_result
            }
            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    fn read_runtime_result(
        &self,
        child: Pid,
        mut reader: PipeReader,
        request_cgroup: &RequestCgroup,
        deadline: std::time::Instant,
    ) -> Result<RuntimeResult> {
        use std::time::Instant;

        Self::set_nonblocking(reader.as_raw_fd()).map_err(|error| FaberError::Generic {
            message: format!("Failed to make runtime result pipe nonblocking: {error}"),
        })?;
        let mut bytes = Vec::new();
        let mut pipe_open = true;
        let mut child_exited = false;

        loop {
            if pipe_open {
                pipe_open = match Self::drain_result_pipe(&mut reader, &mut bytes) {
                    Ok(open) => open,
                    Err(error) => {
                        Self::terminate_request(child, request_cgroup, child_exited);
                        return Err(FaberError::Generic {
                            message: format!("Failed to read runtime result: {error}"),
                        });
                    }
                };
            }
            if !child_exited {
                match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
                    Ok(WaitStatus::StillAlive) => {}
                    Ok(_) | Err(nix::errno::Errno::ECHILD) => child_exited = true,
                    Err(error) => {
                        Self::terminate_request(child, request_cgroup, false);
                        return Err(FaberError::WaitPid { e: error });
                    }
                }
            }
            if !pipe_open && child_exited {
                break;
            }
            if child_exited && pipe_open {
                // The execution child is gone but a descendant still holds
                // the result pipe; nothing further will be written to it.
                Self::terminate_request(child, request_cgroup, true);
            }

            if self.cancellation.is_cancelled() {
                Self::terminate_request(child, request_cgroup, child_exited);
                return Err(FaberError::Cancelled);
            }
            if Self::is_shutting_down() {
                Self::terminate_request(child, request_cgroup, child_exited);
                return Err(FaberError::ShuttingDown);
            }

            let now = Instant::now();
            if now >= deadline {
                Self::terminate_request(child, request_cgroup, child_exited);
                return Err(FaberError::TaskTimeout {
                    timeout_duration: self.overall_timeout,
                    details: "Execution exceeded the hard overall deadline".to_string(),
                });
            }

            // Sleep until the pipe is readable or the deadline passes. The
            // wait is capped so that child exit is noticed promptly.
            let wait = (deadline - now).min(Duration::from_millis(50));
            let mut poll_fd = libc::pollfd {
                fd: reader.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let (poll_fds, count, timeout) = if pipe_open {
                (
                    &mut poll_fd as *mut libc::pollfd,
                    1,
                    wait.as_millis() as i32,
                )
            } else {
                (std::ptr::null_mut(), 0, 1)
            };
            unsafe { libc::poll(poll_fds, count, timeout.max(1)) };
        }

        serde_json::from_slice(&bytes).map_err(|e| FaberError::ParseResult {
            e,
            details: "Failed to parse results from child process".to_string(),
        })
    }

    /// Read everything currently available. Returns whether the pipe is
    /// still open.
    fn drain_result_pipe(reader: &mut PipeReader, bytes: &mut Vec<u8>) -> std::io::Result<bool> {
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => return Ok(false),
                Ok(read) => bytes.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(true),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }

    /// Kill the execution child, its process group (which contains the PID
    /// namespace init) and this request's cgroup subtree, then reap the child.
    /// Other requests live in other request cgroups and are unaffected.
    fn terminate_request(child: Pid, request_cgroup: &RequestCgroup, child_reaped: bool) {
        let _ = nix::sys::signal::kill(
            Pid::from_raw(-child.as_raw()),
            nix::sys::signal::Signal::SIGKILL,
        );
        if !child_reaped {
            let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
        }
        request_cgroup.kill();
        if !child_reaped {
            let _ = waitpid(child, None);
        }
    }

    fn execution_child(
        &self,
        request_cgroup_path: &Path,
        deadline: std::time::Instant,
    ) -> RuntimeResult {
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
            Ok(ForkResult::Child) => Self::run_namespace_init(),
            Ok(ForkResult::Parent { child }) => child,
            Err(e) => {
                return RuntimeResult::ContainerSetupFailed {
                    error: format!("Failed to fork namespace init process: {}", e),
                };
            }
        };

        let mut results = Vec::with_capacity(self.task_group.len());
        // Output kept for the whole request. Every copy of the result (this
        // process, the controller, the API response, the cache) is bounded by
        // it; once spent, later tasks get no output budget at all.
        let mut output_budget = self.request_output_limit;

        for step in &self.task_group {
            // Enforce the overall deadline here so that completed steps are
            // returned: a running step's wall timeout is clipped to what is
            // left, and steps that cannot start are reported as not started.
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                results.push(Self::not_started(step));
                continue;
            }
            let width = match step {
                ExecutionStep::Single(_) => 1,
                ExecutionStep::Parallel(tasks) => tasks.len().max(1),
            };
            let limits = StepLimits {
                timeout: self.timeout.min(remaining),
                output: OutputLimits {
                    per_stream: self.output_limit,
                    total: output_budget / width,
                },
            };
            let result = match step {
                ExecutionStep::Single(task) => {
                    self.execute_single(task.clone(), request_cgroup_path, limits)
                }
                ExecutionStep::Parallel(tasks) => {
                    self.execute_parallel(tasks.clone(), request_cgroup_path, limits)
                }
            };
            output_budget = output_budget.saturating_sub(Self::output_bytes(&result));
            results.push(result);
        }

        // Tear down the namespace init process
        let _ = nix::sys::signal::kill(init_pid, nix::sys::signal::Signal::SIGKILL);
        let _ = waitpid(init_pid, None);

        RuntimeResult::Success(results)
    }

    fn run_namespace_init() -> ! {
        loop {
            match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) | Err(nix::errno::Errno::ECHILD) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(_) => {}
                Err(nix::errno::Errno::EINTR) => {}
                Err(error) => {
                    let _ = error;
                    Self::child_exit(125);
                }
            }
        }
    }

    fn output_bytes(result: &ExecutionStepResult) -> usize {
        let task_bytes = |result: &TaskResult| match result {
            TaskResult::Completed { stdout, stderr, .. } => stdout.len() + stderr.len(),
            TaskResult::Failed { .. } => 0,
        };
        match result {
            ExecutionStepResult::Single(result) => task_bytes(result),
            ExecutionStepResult::Parallel(results) => results.iter().map(task_bytes).sum(),
        }
    }

    fn not_started(step: &ExecutionStep) -> ExecutionStepResult {
        let not_started = || TaskResult::Failed {
            error: "Not started: the overall execution deadline was reached".to_string(),
            stats: TaskResultStats {
                outcome: TaskOutcome::NotStarted,
                cleanup_succeeded: true,
                ..TaskResultStats::default()
            },
        };
        match step {
            ExecutionStep::Single(_) => ExecutionStepResult::Single(not_started()),
            ExecutionStep::Parallel(tasks) => {
                ExecutionStepResult::Parallel(tasks.iter().map(|_| not_started()).collect())
            }
        }
    }

    fn execute_single(
        &self,
        task: Task,
        request_cgroup_path: &Path,
        limits: StepLimits,
    ) -> ExecutionStepResult {
        match Self::execute_single_task(
            task,
            &self.cgroup,
            limits.timeout,
            self.cpu_time_limit,
            limits.output,
            request_cgroup_path,
        ) {
            Ok(task_result) => ExecutionStepResult::Single(task_result),
            Err(e) => ExecutionStepResult::Single(TaskResult::Failed {
                error: format!("Task execution failed: {}", e),
                stats: TaskResultStats::default(),
            }),
        }
    }

    fn execute_parallel(
        &self,
        tasks: Vec<Task>,
        request_cgroup_path: &Path,
        limits: StepLimits,
    ) -> ExecutionStepResult {
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
                    let result = match Self::execute_single_task(
                        task,
                        &self.cgroup,
                        limits.timeout,
                        self.cpu_time_limit,
                        limits.output,
                        request_cgroup_path,
                    ) {
                        Ok(task_result) => task_result,
                        Err(e) => TaskResult::Failed {
                            error: format!("Task execution failed: {}", e),
                            stats: TaskResultStats::default(),
                        },
                    };
                    Self::write_child_result(writer, &result);
                    Self::child_exit(0);
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
        for (child, mut reader) in children {
            // Drain each result pipe before waiting so large bounded outputs do
            // not block the child while it writes. Read in bulk: parsing
            // straight from the unbuffered pipe costs one syscall per byte.
            let mut bytes = Vec::new();
            let result: TaskResult = reader
                .read_to_end(&mut bytes)
                .ok()
                .and_then(|_| serde_json::from_slice(&bytes).ok())
                .unwrap_or(TaskResult::Failed {
                    error: "Failed to read result from parallel task".to_string(),
                    stats: TaskResultStats::default(),
                });
            let _ = waitpid(child, None);
            task_results.push(result);
        }

        ExecutionStepResult::Parallel(task_results)
    }

    fn execute_single_task(
        task: Task,
        cgroup: &Cgroup,
        timeout: std::time::Duration,
        cpu_time_limit: std::time::Duration,
        output_limits: OutputLimits,
        request_cgroup_path: &Path,
    ) -> Result<TaskResult> {
        use std::time::Instant;

        let start_time = Instant::now();

        // Create task cgroup before fork
        let task_cgroup = cgroup.create_task_cgroup(request_cgroup_path)?;

        // Materialize files relative to the workspace without following links.
        // This happens before privilege dropping, so path resolution must fail closed.
        for (file_path, file_content) in task.files.clone().unwrap_or_default() {
            Self::write_workspace_file(&file_path, file_content.as_bytes())?;
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
        let (user_ready_read, user_ready_write) = pipe().map_err(|e| FaberError::MkPipe {
            e: std::io::Error::from_raw_os_error(e as i32),
            details: "Failed to create user namespace ready pipe".to_string(),
        })?;
        let (user_continue_read, user_continue_write) = pipe().map_err(|e| FaberError::MkPipe {
            e: std::io::Error::from_raw_os_error(e as i32),
            details: "Failed to create user namespace continue pipe".to_string(),
        })?;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                let sandbox_profile = task.sandbox_profile.unwrap_or_default();

                // FIRST: Add self to cgroup BEFORE any other work
                // This ensures resource limits apply from the start
                let my_pid = std::process::id();
                if task_cgroup.add_process(my_pid).is_err() {
                    Self::child_exit(127);
                }

                let proc_pid = match std::fs::read_link("/proc/self")
                    .ok()
                    .and_then(|path| path.to_string_lossy().parse::<u32>().ok())
                {
                    Some(pid) => pid,
                    None => Self::child_exit(126),
                };

                // Close read ends of pipes in child
                drop(stdout_read);
                drop(stderr_read);
                drop(stdin_write);
                drop(user_ready_read);
                drop(user_continue_write);

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
                if Self::child_setup_security(
                    cpu_time_limit,
                    user_ready_write.into(),
                    user_continue_read.into(),
                    proc_pid,
                    sandbox_profile,
                )
                .is_err()
                {
                    Self::child_exit(126);
                }

                // Change working directory if specified
                if let Some(ref working_dir) = task.working_dir {
                    let dir_cstr = match CString::new(working_dir.clone()) {
                        Ok(c) => c,
                        Err(_) => {
                            Self::child_exit(127);
                        }
                    };
                    if chdir(dir_cstr.as_c_str()).is_err() {
                        Self::child_exit(127);
                    }
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
                    Err(_) => Self::child_exit(127),
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
                Self::child_exit(127);
            }
            Ok(ForkResult::Parent { child }) => {
                // Close write ends of pipes in parent
                drop(stdout_write);
                drop(stderr_write);
                drop(stdin_read);
                drop(user_ready_write);
                drop(user_continue_read);

                Self::configure_child_user_namespace(
                    child,
                    user_ready_read.into(),
                    user_continue_write.into(),
                )?;

                let collected = Self::wait_and_collect_output(
                    child,
                    timeout,
                    stdout_read.into(),
                    stderr_read.into(),
                    stdin_write.into(),
                    task.stdin.unwrap_or_default().into_bytes(),
                    output_limits,
                    &task_cgroup,
                )?;

                // Measure resources
                let task_stats = task_cgroup.measure_resources().unwrap_or_default();

                let events = task_cgroup.measure_events();
                let cleanup_succeeded = match task_cgroup.cleanup() {
                    Ok(()) => true,
                    Err(_) => false,
                };
                let outcome = if collected.timed_out {
                    TaskOutcome::TimedOut
                } else if collected.output_terminated {
                    TaskOutcome::OutputLimit
                } else if events.oom_kill_count > 0 && events.own_oom_count > 0 {
                    TaskOutcome::OutOfMemory
                } else if events.oom_kill_count > 0 {
                    TaskOutcome::AncestorOutOfMemory
                } else if events.pids_limit_hit_count > 0 {
                    TaskOutcome::PidsLimit
                } else if collected.termination_signal == Some(libc::SIGSYS) {
                    TaskOutcome::PolicyViolation
                } else if collected.termination_signal.is_some() {
                    TaskOutcome::Signaled
                } else {
                    TaskOutcome::Exited
                };

                let stats = TaskResultStats {
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                    memory_peak_bytes: task_stats.memory_peak_bytes,
                    cpu_usage_usec: task_stats.cpu_usage_usec,
                    cpu_nr_throttled: task_stats.cpu_nr_throttled,
                    cpu_throttled_usec: task_stats.cpu_throttled_usec,
                    pids_peak: task_stats.pids_max,
                    stdout_truncated: collected.stdout_truncated,
                    stderr_truncated: collected.stderr_truncated,
                    outcome,
                    termination_signal: collected.termination_signal,
                    oom_kill_count: events.oom_kill_count,
                    pids_limit_hit_count: events.pids_limit_hit_count,
                    cleanup_succeeded,
                };

                Ok(TaskResult::Completed {
                    stdout: String::from_utf8_lossy(&collected.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&collected.stderr).into_owned(),
                    exit_code: collected.exit_code,
                    stats,
                })
            }
            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    /// Serialize a result to a pipe through a buffer; unbuffered serde writes
    /// issue one syscall per token.
    fn write_child_result<T: serde::Serialize>(writer: PipeWriter, result: &T) {
        let mut writer = std::io::BufWriter::with_capacity(64 * 1024, writer);
        if serde_json::to_writer(&mut writer, result).is_ok() {
            let _ = writer.flush();
        }
    }

    fn child_exit(code: i32) -> ! {
        unsafe { libc::_exit(code) }
    }

    fn write_workspace_file(file_path: &str, content: &[u8]) -> Result<()> {
        // Submitted files and the directories created for them belong to the
        // task identity, so the task can modify and extend nested layouts.
        const TASK_ID: libc::uid_t = 65534;

        let path = Path::new(file_path);
        if file_path.is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(FaberError::InvalidTaskFilePath {
                path: file_path.to_string(),
                details: "paths must be normalized and relative to the workspace".to_string(),
            });
        }
        let component_cstr = |component: &std::ffi::OsStr| {
            CString::new(component.as_bytes()).map_err(|_| FaberError::InvalidTaskFilePath {
                path: file_path.to_string(),
                details: "paths cannot contain NUL bytes".to_string(),
            })
        };
        let components: Vec<&std::ffi::OsStr> = path
            .components()
            .map(|component| component.as_os_str())
            .collect();
        let Some((file_name, directories)) = components.split_last() else {
            unreachable!("task path was checked to be non-empty");
        };

        let workspace = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open(".")
            .map_err(|e| FaberError::WriteFile {
                e,
                details: "Failed to open the task workspace".to_string(),
            })?;

        // Walk one component at a time. Each directory is created and then
        // opened relative to the descriptor of its already verified parent,
        // so a concurrent task swapping a component for a symlink cannot make
        // this root-privileged walk resolve anywhere else.
        let mut parent = std::os::fd::OwnedFd::from(workspace);
        let mut created_path = std::path::PathBuf::new();
        for directory in directories {
            created_path.push(directory);
            let name = component_cstr(directory)?;
            let created = unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o755) } == 0;
            if !created
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(FaberError::WriteFile {
                    e: std::io::Error::last_os_error(),
                    details: format!(
                        "Failed to create task directory '{}'",
                        created_path.display()
                    ),
                });
            }

            let directory_fd = Self::open_beneath(
                &parent,
                &name,
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                0,
            )
            .map_err(|e| FaberError::WriteFile {
                e,
                details: format!(
                    "Refused task directory '{}' because it is not safely beneath the workspace without following links",
                    created_path.display()
                ),
            })?;
            if created && unsafe { libc::fchown(directory_fd.as_raw_fd(), TASK_ID, TASK_ID) } != 0 {
                return Err(FaberError::WriteFile {
                    e: std::io::Error::last_os_error(),
                    details: format!(
                        "Failed to hand task directory '{}' to the task user",
                        created_path.display()
                    ),
                });
            }
            parent = directory_fd;
        }

        let name = component_cstr(file_name)?;
        let file_fd = Self::open_beneath(
            &parent,
            &name,
            libc::O_WRONLY
                | libc::O_CREAT
                | libc::O_TRUNC
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK,
            0o644,
        )
        .map_err(|e| FaberError::WriteFile {
            e,
            details: format!(
                "Refused to open task file '{file_path}' beneath the workspace without following links"
            ),
        })?;

        let mut file = std::fs::File::from(file_fd);
        let metadata = file.metadata().map_err(|e| FaberError::WriteFile {
            e,
            details: format!("Failed to inspect task file '{file_path}'"),
        })?;
        if !metadata.is_file() {
            return Err(FaberError::InvalidTaskFilePath {
                path: file_path.to_string(),
                details: "task file targets must be regular files".to_string(),
            });
        }
        if unsafe { libc::fchown(file.as_raw_fd(), TASK_ID, TASK_ID) } != 0 {
            return Err(FaberError::WriteFile {
                e: std::io::Error::last_os_error(),
                details: format!("Failed to hand task file '{file_path}' to the task user"),
            });
        }

        file.write_all(content).map_err(|e| FaberError::WriteFile {
            e,
            details: format!("Failed to write task file '{file_path}'"),
        })?;

        Ok(())
    }

    /// Open a single path component beneath `parent` with openat2(2), refusing
    /// symlinks, magic links and mount crossings.
    fn open_beneath(
        parent: &std::os::fd::OwnedFd,
        name: &CString,
        flags: i32,
        mode: u64,
    ) -> std::io::Result<std::os::fd::OwnedFd> {
        #[repr(C)]
        struct OpenHow {
            flags: u64,
            mode: u64,
            resolve: u64,
        }

        // Linux openat2(2) resolve flags. Keep these local until libc exposes a
        // stable open_how type across all supported build targets.
        const RESOLVE_NO_XDEV: u64 = 0x01;
        const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
        const RESOLVE_NO_SYMLINKS: u64 = 0x04;
        const RESOLVE_BENEATH: u64 = 0x08;

        let how = OpenHow {
            flags: flags as u64,
            mode,
            resolve: RESOLVE_NO_XDEV
                | RESOLVE_NO_MAGICLINKS
                | RESOLVE_NO_SYMLINKS
                | RESOLVE_BENEATH,
        };
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                parent.as_raw_fd(),
                name.as_ptr(),
                &how,
                std::mem::size_of::<OpenHow>(),
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd as i32) })
    }

    /// Set up security restrictions in child process before exec
    fn child_setup_security(
        cpu_time_limit: Duration,
        user_ready: PipeWriter,
        user_continue: PipeReader,
        proc_pid: u32,
        sandbox_profile: SandboxProfile,
    ) -> std::io::Result<()> {
        let unshare_flags = CloneFlags::CLONE_NEWNS;

        unshare(unshare_flags).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // mask_paths unmounts and masks proc/sys with tmpfs for security
        Container::mask_paths()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // Mount proc from inside the PID namespace. The dedicated namespace
        // init is PID 1; this task is one of its descendants.
        Self::mount_proc_in_pid_namespace()?;

        // Mount sys from oldroot (sysfs doesn't have PID-specific info)
        Self::mount_sys()?;
        Self::apply_resource_limits(cpu_time_limit)?;

        setgroups(&[]).map_err(std::io::Error::other)?;
        Self::enter_user_namespace(user_ready, user_continue, proc_pid)?;

        // Clear every capability set granted while establishing the new user
        // namespace before executing submitted code.
        Self::clear_linux_capability_sets()?;
        Self::drop_posix_capabilities()?;
        Self::set_no_new_privileges()?;
        Self::apply_seccomp_filter(sandbox_profile)?;

        Ok(())
    }

    /// Mount proc filesystem in the child's PID namespace
    /// This MUST be called after the child enters the PID namespace
    fn mount_proc_in_pid_namespace() -> std::io::Result<()> {
        use nix::mount::{MsFlags, mount};

        // Create /proc directory
        std::fs::create_dir_all("/proc").ok();

        // Mount a fresh proc filesystem
        // This will show only the processes in the current PID namespace
        // because we're calling this from the child that is PID 1 in the new namespace
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;
        mount(
            Some("proc"),
            "/proc",
            Some("proc"),
            proc_flags,
            None::<&str>,
        )
        .map_err(|e| std::io::Error::other(format!("Failed to mount procfs: {e}")))?;

        Ok(())
    }

    /// Mount sysfs in the new mount namespace
    fn mount_sys() -> std::io::Result<()> {
        use nix::mount::{MsFlags, mount};

        std::fs::create_dir_all("/sys").ok();

        let sys_flags =
            MsFlags::MS_RDONLY | MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;
        mount(None::<&str>, "/sys", Some("sysfs"), sys_flags, None::<&str>)
            .map_err(|error| std::io::Error::other(format!("Failed to mount sysfs: {error}")))?;
        mount(
            None::<&str>,
            "/sys",
            None::<&str>,
            sys_flags | MsFlags::MS_REMOUNT,
            None::<&str>,
        )
        .map_err(|error| {
            std::io::Error::other(format!("Failed to remount sysfs read-only: {error}"))
        })?;

        Ok(())
    }

    fn set_nonblocking(fd: i32) -> std::io::Result<()> {
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    fn drain_pipe(
        reader: &mut PipeReader,
        output: &mut Vec<u8>,
        output_limit: usize,
        truncated: &mut bool,
    ) -> std::io::Result<bool> {
        let mut chunk = [0u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => return Ok(false),
                Ok(bytes_read) => {
                    let remaining = output_limit.saturating_sub(output.len());
                    let retained = remaining.min(bytes_read);
                    output.extend_from_slice(&chunk[..retained]);
                    if retained < bytes_read {
                        *truncated = true;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(true),
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
    }

    fn wait_status_result(status: WaitStatus) -> Option<(i32, Option<i32>)> {
        match status {
            WaitStatus::Exited(_, code) => Some((code, None)),
            WaitStatus::Signaled(_, signal, _) => Some((128 + signal as i32, Some(signal as i32))),
            _ => None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wait_and_collect_output(
        child: Pid,
        timeout: Duration,
        mut stdout_reader: PipeReader,
        mut stderr_reader: PipeReader,
        stdin_writer: PipeWriter,
        stdin: Vec<u8>,
        output_limits: OutputLimits,
        task_cgroup: &TaskCgroup,
    ) -> Result<CollectedOutput> {
        use std::time::Instant;

        Self::set_nonblocking(stdout_reader.as_raw_fd()).map_err(|error| FaberError::Generic {
            message: format!("Failed to make stdout nonblocking: {error}"),
        })?;
        Self::set_nonblocking(stderr_reader.as_raw_fd()).map_err(|error| FaberError::Generic {
            message: format!("Failed to make stderr nonblocking: {error}"),
        })?;
        Self::set_nonblocking(stdin_writer.as_raw_fd()).map_err(|error| FaberError::Generic {
            message: format!("Failed to make stdin nonblocking: {error}"),
        })?;

        let start_time = Instant::now();
        let mut stdout = Vec::with_capacity(output_limits.per_stream.min(8192));
        let mut stderr = Vec::with_capacity(output_limits.per_stream.min(8192));
        let mut stdout_open = true;
        let mut stderr_open = true;
        let mut stdin_writer = Some(stdin_writer);
        let mut stdin_offset = 0;
        let mut exit_code = None;
        let mut stdout_truncated = false;
        let mut stderr_truncated = false;
        let mut output_terminated = false;
        let mut timed_out = false;
        let mut termination_signal = None;
        let mut kill_started_at: Option<Instant> = None;

        loop {
            if stdin_offset == stdin.len() {
                stdin_writer = None;
            }

            let mut poll_fds = Vec::with_capacity(3);
            if stdout_open {
                poll_fds.push(libc::pollfd {
                    fd: stdout_reader.as_raw_fd(),
                    events: libc::POLLIN | libc::POLLHUP,
                    revents: 0,
                });
            }
            if stderr_open {
                poll_fds.push(libc::pollfd {
                    fd: stderr_reader.as_raw_fd(),
                    events: libc::POLLIN | libc::POLLHUP,
                    revents: 0,
                });
            }
            if let Some(writer) = stdin_writer.as_ref() {
                poll_fds.push(libc::pollfd {
                    fd: writer.as_raw_fd(),
                    events: libc::POLLOUT | libc::POLLHUP,
                    revents: 0,
                });
            }

            let poll_result =
                unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as libc::nfds_t, 10) };
            if poll_result < 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
            {
                return Err(FaberError::Generic {
                    message: format!(
                        "Failed to poll task pipes: {}",
                        std::io::Error::last_os_error()
                    ),
                });
            }

            if stdout_open {
                stdout_open = Self::drain_pipe(
                    &mut stdout_reader,
                    &mut stdout,
                    output_limits
                        .per_stream
                        .min(output_limits.total.saturating_sub(stderr.len())),
                    &mut stdout_truncated,
                )
                .map_err(|error| FaberError::Generic {
                    message: format!("Failed to read task stdout: {error}"),
                })?;
            }
            if stderr_open {
                stderr_open = Self::drain_pipe(
                    &mut stderr_reader,
                    &mut stderr,
                    output_limits
                        .per_stream
                        .min(output_limits.total.saturating_sub(stdout.len())),
                    &mut stderr_truncated,
                )
                .map_err(|error| FaberError::Generic {
                    message: format!("Failed to read task stderr: {error}"),
                })?;
            }

            if let Some(writer) = stdin_writer.as_mut() {
                match writer.write(&stdin[stdin_offset..]) {
                    Ok(0) => stdin_writer = None,
                    Ok(bytes_written) => stdin_offset += bytes_written,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => {
                        stdin_writer = None;
                    }
                    Err(error) => {
                        return Err(FaberError::Generic {
                            message: format!("Failed to write task stdin: {error}"),
                        });
                    }
                }
            }

            if !output_terminated && (stdout_truncated || stderr_truncated) {
                task_cgroup.kill_all_processes()?;
                stdin_writer = None;
                output_terminated = true;
                kill_started_at = Some(Instant::now());
            }

            if exit_code.is_none() {
                match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
                    Ok(WaitStatus::StillAlive) => {}
                    Ok(status) => {
                        if let Some((code, signal)) = Self::wait_status_result(status) {
                            exit_code = Some(code);
                            termination_signal = signal;
                        }
                    }
                    Err(nix::errno::Errno::ECHILD) => exit_code = Some(-1),
                    Err(error) => return Err(FaberError::WaitPid { e: error }),
                }
            }

            if exit_code.is_some() && !stdout_open && !stderr_open {
                break;
            }

            if kill_started_at.is_some()
                && (!task_cgroup.is_populated()
                    || kill_started_at
                        .is_some_and(|started| started.elapsed() >= Duration::from_millis(500)))
            {
                if exit_code.is_none() {
                    let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
                    if let Ok(status) = waitpid(child, None)
                        && let Some((code, signal)) = Self::wait_status_result(status)
                    {
                        exit_code = Some(code);
                        termination_signal = signal;
                    }
                }
                break;
            }

            if !timed_out && start_time.elapsed() > timeout {
                task_cgroup.kill_all_processes()?;
                stdin_writer = None;
                timed_out = true;
                kill_started_at = Some(Instant::now());
            }
        }

        Ok(CollectedOutput {
            stdout,
            stderr,
            exit_code: exit_code.unwrap_or(-1),
            stdout_truncated,
            stderr_truncated,
            termination_signal,
            timed_out,
            output_terminated,
        })
    }

    fn configure_child_user_namespace(
        child: Pid,
        mut user_ready: PipeReader,
        mut user_continue: PipeWriter,
    ) -> Result<()> {
        let mut ready = [0; std::mem::size_of::<u32>()];
        if let Err(error) = user_ready.read_exact(&mut ready) {
            let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
            let _ = waitpid(child, None);
            return Err(FaberError::Generic {
                message: format!("Task failed before entering its user namespace: {error}"),
            });
        }
        let proc_pid = u32::from_ne_bytes(ready);
        if proc_pid == 0 {
            let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
            let _ = waitpid(child, None);
            return Err(FaberError::Generic {
                message: "Task reported an invalid user namespace handshake".to_string(),
            });
        }

        let proc_path = Path::new("/proc").join(proc_pid.to_string());
        let mapping_result = (|| -> std::io::Result<()> {
            std::fs::write(proc_path.join("setgroups"), "deny").map_err(|error| {
                std::io::Error::new(error.kind(), format!("setgroups: {error}"))
            })?;
            std::fs::write(proc_path.join("uid_map"), "65534 65534 1\n")
                .map_err(|error| std::io::Error::new(error.kind(), format!("uid_map: {error}")))?;
            std::fs::write(proc_path.join("gid_map"), "65534 65534 1\n")
                .map_err(|error| std::io::Error::new(error.kind(), format!("gid_map: {error}")))?;
            Ok(())
        })();

        let configured = u8::from(mapping_result.is_ok());
        let _ = user_continue.write_all(&[configured]);
        if let Err(error) = mapping_result {
            let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
            let _ = waitpid(child, None);
            return Err(FaberError::Generic {
                message: format!("Failed to configure task user namespace: {error}"),
            });
        }

        let identity_result = (|| -> std::io::Result<()> {
            let mut identity_ready = [0];
            user_ready.read_exact(&mut identity_ready)?;
            if identity_ready != [1] {
                return Err(std::io::Error::other("invalid identity confirmation"));
            }
            let status = std::fs::read_to_string(proc_path.join("status"))?;
            for field in ["Uid:", "Gid:"] {
                let values = status
                    .lines()
                    .find(|line| line.starts_with(field))
                    .ok_or_else(|| std::io::Error::other(format!("missing {field}")))?
                    .split_whitespace()
                    .skip(1)
                    .map(str::parse::<u32>)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(std::io::Error::other)?;
                if values != [65534, 65534, 65534, 65534] {
                    return Err(std::io::Error::other(format!(
                        "unexpected outer {field} values: {values:?}"
                    )));
                }
            }
            Ok(())
        })();
        if let Err(error) = identity_result {
            let _ = nix::sys::signal::kill(child, nix::sys::signal::Signal::SIGKILL);
            let _ = waitpid(child, None);
            return Err(FaberError::Generic {
                message: format!("Failed to verify task user namespace identity: {error}"),
            });
        }

        Ok(())
    }

    fn enter_user_namespace(
        mut user_ready: PipeWriter,
        mut user_continue: PipeReader,
        proc_pid: u32,
    ) -> std::io::Result<()> {
        const TASK_ID: u32 = 65534;

        unshare(CloneFlags::CLONE_NEWUSER).map_err(std::io::Error::other)?;
        user_ready.write_all(&proc_pid.to_ne_bytes())?;
        let mut configured = [0];
        user_continue.read_exact(&mut configured)?;
        if configured != [1] {
            return Err(std::io::Error::other("user namespace mapping failed"));
        }

        setgid(TASK_ID.into()).map_err(std::io::Error::other)?;
        setuid(TASK_ID.into()).map_err(std::io::Error::other)?;

        if unsafe { libc::getuid() } != TASK_ID || unsafe { libc::getgid() } != TASK_ID {
            return Err(std::io::Error::other(
                "task UID/GID do not match the configured user namespace maps",
            ));
        }
        user_ready.write_all(&[1])?;

        Ok(())
    }

    fn set_resource_limit(resource: RlimitResource, value: u64) -> std::io::Result<()> {
        let limit = libc::rlimit {
            rlim_cur: value,
            rlim_max: value,
        };
        if unsafe { libc::setrlimit(resource, &limit) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    fn apply_resource_limits(cpu_time_limit: Duration) -> std::io::Result<()> {
        const FILE_SIZE_LIMIT: u64 = 64 * 1024 * 1024;
        const OPEN_FILE_LIMIT: u64 = 256;
        const STACK_LIMIT: u64 = 8 * 1024 * 1024;

        let cpu_seconds = cpu_time_limit.as_secs().max(1);
        Self::set_resource_limit(libc::RLIMIT_CPU, cpu_seconds)?;
        Self::set_resource_limit(libc::RLIMIT_FSIZE, FILE_SIZE_LIMIT)?;
        Self::set_resource_limit(libc::RLIMIT_NOFILE, OPEN_FILE_LIMIT)?;
        Self::set_resource_limit(libc::RLIMIT_STACK, STACK_LIMIT)?;
        Self::set_resource_limit(libc::RLIMIT_CORE, 0)
    }

    fn clear_capability_set(capability_set: CapSet, name: &str) -> std::io::Result<()> {
        caps::clear(None, capability_set).map_err(|error| {
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("Failed to clear {name} capabilities: {error}"),
            )
        })
    }

    fn clear_linux_capability_sets() -> std::io::Result<()> {
        Self::clear_capability_set(CapSet::Ambient, "ambient")?;
        Self::clear_capability_set(CapSet::Bounding, "bounding")
    }

    fn drop_posix_capabilities() -> std::io::Result<()> {
        Self::clear_capability_set(CapSet::Effective, "effective")?;
        Self::clear_capability_set(CapSet::Permitted, "permitted")?;
        Self::clear_capability_set(CapSet::Inheritable, "inheritable")
    }

    fn set_no_new_privileges() -> std::io::Result<()> {
        let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    fn apply_seccomp_filter(profile: SandboxProfile) -> std::io::Result<()> {
        use seccompiler::{
            BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition,
            SeccompFilter, SeccompRule,
        };
        use std::collections::BTreeMap;

        #[cfg(target_arch = "x86_64")]
        Self::apply_x32_seccomp_guard()?;

        if profile == SandboxProfile::CompileV1 {
            let architecture = std::env::consts::ARCH.try_into().map_err(|error| {
                std::io::Error::other(format!("unsupported seccomp architecture: {error}"))
            })?;
            let filter = SeccompFilter::new(
                BTreeMap::from([(libc::SYS_clone3, Vec::new())]),
                SeccompAction::Allow,
                SeccompAction::Errno(libc::ENOSYS as u32),
                architecture,
            )
            .map_err(|error| {
                std::io::Error::other(format!("failed to compile clone3 filter: {error}"))
            })?;
            let program: BpfProgram = filter.try_into().map_err(|error| {
                std::io::Error::other(format!("failed to compile clone3 BPF: {error}"))
            })?;
            seccompiler::apply_filter(&program).map_err(|error| {
                std::io::Error::other(format!("failed to apply clone3 filter: {error}"))
            })?;
        }

        let mut blocked_syscalls = vec![
            libc::SYS_acct,
            libc::SYS_add_key,
            libc::SYS_bpf,
            libc::SYS_delete_module,
            libc::SYS_finit_module,
            libc::SYS_fanotify_init,
            libc::SYS_fsconfig,
            libc::SYS_fsmount,
            libc::SYS_fsopen,
            libc::SYS_init_module,
            libc::SYS_io_uring_setup,
            libc::SYS_kcmp,
            libc::SYS_kexec_load,
            libc::SYS_kexec_file_load,
            libc::SYS_keyctl,
            libc::SYS_mount,
            libc::SYS_mount_setattr,
            libc::SYS_move_mount,
            libc::SYS_name_to_handle_at,
            libc::SYS_open_tree,
            libc::SYS_open_by_handle_at,
            libc::SYS_perf_event_open,
            libc::SYS_pidfd_getfd,
            libc::SYS_pivot_root,
            libc::SYS_process_vm_readv,
            libc::SYS_process_vm_writev,
            libc::SYS_ptrace,
            libc::SYS_quotactl,
            libc::SYS_reboot,
            libc::SYS_request_key,
            libc::SYS_setns,
            libc::SYS_swapoff,
            libc::SYS_swapon,
            libc::SYS_umount2,
            libc::SYS_unshare,
            libc::SYS_userfaultfd,
        ];
        if profile == SandboxProfile::NativeV1 {
            blocked_syscalls.extend([
                libc::SYS_clone,
                libc::SYS_clone3,
                libc::SYS_socket,
                libc::SYS_socketpair,
            ]);
            #[cfg(target_arch = "x86_64")]
            blocked_syscalls.extend([libc::SYS_fork, libc::SYS_vfork]);
        }

        if profile == SandboxProfile::CompileV1 {
            let clone_rules = [
                libc::CLONE_NEWCGROUP,
                libc::CLONE_NEWIPC,
                libc::CLONE_NEWNET,
                libc::CLONE_NEWNS,
                libc::CLONE_NEWPID,
                libc::CLONE_NEWTIME,
                libc::CLONE_NEWUSER,
                libc::CLONE_NEWUTS,
            ]
            .into_iter()
            .map(|flag| {
                let flag = flag as u64;
                let condition = SeccompCondition::new(
                    0,
                    SeccompCmpArgLen::Qword,
                    SeccompCmpOp::MaskedEq(flag),
                    flag,
                )?;
                SeccompRule::new(vec![condition])
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|error| {
                std::io::Error::other(format!("failed to build clone flag rules: {error}"))
            })?;
            blocked_syscalls.push(libc::SYS_clone);
            let mut rules: BTreeMap<i64, Vec<SeccompRule>> = blocked_syscalls
                .into_iter()
                .map(|syscall| (syscall, Vec::new()))
                .collect();
            rules.insert(libc::SYS_clone, clone_rules);
            return Self::compile_and_apply_seccomp(rules);
        }

        let rules: BTreeMap<i64, Vec<SeccompRule>> = blocked_syscalls
            .into_iter()
            .map(|syscall| (syscall, Vec::new()))
            .collect();
        Self::compile_and_apply_seccomp(rules)
    }

    fn compile_and_apply_seccomp(
        rules: std::collections::BTreeMap<i64, Vec<seccompiler::SeccompRule>>,
    ) -> std::io::Result<()> {
        use seccompiler::{BpfProgram, SeccompAction, SeccompFilter};

        let architecture = std::env::consts::ARCH.try_into().map_err(|error| {
            std::io::Error::other(format!("unsupported seccomp architecture: {error}"))
        })?;
        let filter = SeccompFilter::new(
            rules,
            SeccompAction::Allow,
            SeccompAction::Trap,
            architecture,
        )
        .map_err(|error| {
            std::io::Error::other(format!("failed to compile seccomp profile: {error}"))
        })?;
        let program: BpfProgram = filter.try_into().map_err(|error| {
            std::io::Error::other(format!("failed to compile seccomp BPF: {error}"))
        })?;
        seccompiler::apply_filter(&program).map_err(|error| {
            std::io::Error::other(format!("failed to apply seccomp profile: {error}"))
        })
    }

    #[cfg(target_arch = "x86_64")]
    fn apply_x32_seccomp_guard() -> std::io::Result<()> {
        const X32_SYSCALL_BIT: u32 = 0x4000_0000;
        const BPF_LOAD_SYSCALL_NR: u16 = 0x20;
        const BPF_JUMP_GREATER_OR_EQUAL: u16 = 0x35;
        const BPF_RETURN: u16 = 0x06;
        const SECCOMP_RETURN_TRAP: u32 = 0x0003_0000;
        const SECCOMP_RETURN_ALLOW: u32 = 0x7fff_0000;

        let mut instructions = [
            libc::sock_filter {
                code: BPF_LOAD_SYSCALL_NR,
                jt: 0,
                jf: 0,
                k: 0,
            },
            libc::sock_filter {
                code: BPF_JUMP_GREATER_OR_EQUAL,
                jt: 0,
                jf: 1,
                k: X32_SYSCALL_BIT,
            },
            libc::sock_filter {
                code: BPF_RETURN,
                jt: 0,
                jf: 0,
                k: SECCOMP_RETURN_TRAP,
            },
            libc::sock_filter {
                code: BPF_RETURN,
                jt: 0,
                jf: 0,
                k: SECCOMP_RETURN_ALLOW,
            },
        ];
        let program = libc::sock_fprog {
            len: instructions.len() as u16,
            filter: instructions.as_mut_ptr(),
        };
        let result = unsafe {
            libc::prctl(
                libc::PR_SET_SECCOMP,
                libc::SECCOMP_MODE_FILTER,
                &program as *const libc::sock_fprog,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}
