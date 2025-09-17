use std::{
    io::{PipeWriter, Read, Write},
    os::{fd::IntoRawFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio, exit},
};

use caps::{CapSet, Capability};
use nix::{
    sched::{CloneFlags, unshare},
    sys::wait::waitpid,
    unistd::{ForkResult, Group, User, fork, getgid, getuid, setgid, setuid},
};

use crate::{
    ExecutionStep, ExecutionStepResult, Task, TaskGroup, TaskResult, TaskResultStats,
    container::Container,
    prelude::*,
    result::{RuntimeResult, TaskGroupResult},
    utils::{close_fd, mk_pipe},
};

pub struct Runtime {
    task_group: TaskGroup,
    container: Container,
}

impl Runtime {
    pub fn new(task_group: TaskGroup) -> Self {
        let container = Container::default();

        Self {
            task_group,
            container,
        }
    }

    pub fn execute(&self) -> Result<RuntimeResult> {
        let (reader, mut writer) = mk_pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                close_fd(reader.into_raw_fd())?;

                let runtime_result = self.execution_child();
                let _ = serde_json::to_writer(writer, &runtime_result);
                exit(0);
            }
            Ok(ForkResult::Parent { child }) => {
                close_fd(writer.into_raw_fd())?;
                waitpid(child, None);

                let runtime_result: RuntimeResult = match serde_json::from_reader(reader) {
                    Ok(result) => result,
                    Err(e) => {
                        println!("Failed to parse results from child process: {}", e);
                        return Err(FaberError::ParseResult {
                            e,
                            details: "Failed to parse results from child process".to_string(),
                        });
                    }
                };
                if let Err(e) = self.container.cleanup() {
                    eprintln!("Failed to cleanup container: {}", e);
                }

                Ok(runtime_result)
            }
            Err(e) => Err(FaberError::Fork { e }),
        }
    }

    fn execution_child(&self) -> RuntimeResult {
        // Handle container setup separately from task execution
        if let Err(e) = self.container.setup() {
            return RuntimeResult::ContainerSetupFailed {
                error: format!("Container setup failed: {}", e),
            };
        }

        let mut results = Vec::with_capacity(self.task_group.len());

        for step in &self.task_group {
            let result = match step {
                ExecutionStep::Single(task) => Self::execute_single(task.clone()),
                ExecutionStep::Parallel(tasks) => Self::execute_parallel(tasks.clone()),
            };
            results.push(result);
        }

        RuntimeResult::Success(results)
    }

    fn execute_single(task: Task) -> ExecutionStepResult {
        match Self::execute_task(task) {
            Ok(task_result) => ExecutionStepResult::Single(task_result),
            Err(e) => ExecutionStepResult::Single(TaskResult::Failed {
                error: format!("Task execution failed: {}", e),
                stats: TaskResultStats::default(),
            }),
        }
    }

    fn execute_parallel(tasks: Vec<Task>) -> ExecutionStepResult {
        let mut handles = Vec::with_capacity(tasks.len());

        for task in tasks {
            let handle = std::thread::spawn(move || match Self::execute_task(task) {
                Ok(task_result) => task_result,
                Err(e) => TaskResult::Failed {
                    error: format!("Task execution failed: {}", e),
                    stats: TaskResultStats::default(),
                },
            });
            handles.push(handle);
        }

        // Wait for all threads to complete and collect results
        let task_results = Self::collect_parallel_results(handles);
        ExecutionStepResult::Parallel(task_results)
    }

    fn collect_parallel_results(
        handles: Vec<std::thread::JoinHandle<TaskResult>>,
    ) -> Vec<TaskResult> {
        let mut task_results = Vec::with_capacity(handles.len());

        for handle in handles {
            let result = match handle.join() {
                Ok(task_result) => task_result,
                Err(_) => TaskResult::Failed {
                    error: "Thread panicked during task execution".to_string(),
                    stats: TaskResultStats::default(),
                },
            };
            task_results.push(result);
        }

        task_results
    }

    fn execute_task(task: Task) -> Result<TaskResult> {
        let mut cmd = Command::new(task.cmd);

        for (key, value) in task.env.unwrap_or_default() {
            cmd.env(key, value);
        }

        let has_path = cmd.get_envs().any(|(key, _)| key == "PATH");
        if !has_path {
            cmd.env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            );
        }

        if let Some(args) = task.args {
            cmd.args(args);
        }

        for (file_path, file_content) in task.files.unwrap_or_default() {
            let file_path = PathBuf::from(file_path);
            std::fs::write(file_path, file_content).map_err(|e| FaberError::WriteFile {
                e,
                details: "Failed to write file".to_string(),
            })?;
        }

        unsafe { cmd.pre_exec(Self::pre_execute_task) };

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| FaberError::ExecuteTask {
            e,
            details: "Failed to spawn task".to_string(),
        })?;

        if let Some(stdin) = task.stdin
            && let Some(mut child_stdin) = child.stdin.take()
        {
            child_stdin
                .write_all(stdin.as_bytes())
                .map_err(|e| FaberError::WriteStdin {
                    e,
                    details: "Failed to write stdin".to_string(),
                })?;
        }

        let exit_status = child.wait().unwrap();

        let mut stdout = child.stdout.unwrap();
        let mut stderr = child.stderr.unwrap();

        let mut stdout_buf = String::new();
        let mut stderr_buf = String::new();

        stdout.read_to_string(&mut stdout_buf).unwrap();
        stderr.read_to_string(&mut stderr_buf).unwrap();

        Ok(TaskResult::Completed {
            stdout: stdout_buf,
            stderr: stderr_buf,
            exit_code: exit_status.code().unwrap_or(-1),
            stats: TaskResultStats::default(),
        })
    }

    fn pre_execute_task() -> std::io::Result<()> {
        let unshare_flags = CloneFlags::CLONE_NEWNS;

        // Perform privileged operations first (these require capabilities)
        unshare(unshare_flags).unwrap();
        Container::mask_paths().unwrap();

        // Change to unprivileged user/group (requires CAP_SETUID/CAP_SETGID)
        setgid(65534.into()).unwrap();
        setuid(65534.into()).unwrap();

        // Drop all capabilities AFTER all privileged operations are complete
        // This ensures the user command runs with no special privileges
        Self::drop_capabilities().unwrap();

        // Apply seccomp filter to restrict system calls
        Self::apply_seccomp_filter().unwrap();

        Ok(())
    }

    fn drop_capabilities() -> std::io::Result<()> {
        // Clear all capabilities from effective, permitted, and inheritable sets
        // This must happen AFTER all privileged operations are complete
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

    #[allow(dead_code)]
    fn apply_seccomp_filter() -> std::io::Result<()> {
        // Seccomp implementation commented out for now due to dependency complexity
        // This would block dangerous system calls like:
        // - Process creation: clone, fork, vfork, execve, execveat
        // - Namespace manipulation: unshare, setns
        // - Mount operations: mount, umount, umount2, pivot_root
        // - Capability manipulation: capset, capget
        // - User/group changes: setuid, setgid, etc.
        // - Kernel modules: init_module, finit_module, delete_module
        // - System admin: reboot, sethostname, setdomainname
        // - Debugging: ptrace
        // - BPF operations: bpf
        // - Keyring: keyctl, add_key, request_key
        Ok(())
    }
}
