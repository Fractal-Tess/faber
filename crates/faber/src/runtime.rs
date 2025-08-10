use nix::{
    sys::{
        signal::{Signal, kill},
        wait::{WaitStatus, waitpid},
    },
    unistd::{ForkResult, Pid, close, dup2, execve, fork, pipe},
};
use rand::{Rng, distr::Alphanumeric};

use crate::{
    TaskResult,
    builder::RuntimeBuilder,
    cgroups::{CgroupHandle, Cgroups},
    environment::ContainerEnvironment,
    prelude::*,
    types::Task,
};

use std::{
    ffi::CString,
    fs::File,
    io::Read,
    mem::ManuallyDrop,
    os::fd::{FromRawFd, IntoRawFd, OwnedFd},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Debug)]
pub struct Runtime {
    pub(crate) env: ContainerEnvironment,
    pub(crate) cgroups: Cgroups,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn run(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        let (results_read_fd, results_write_fd) = pipe()?;
        eprintln!(
            "[faber:run] container_root={}, tasks={}",
            self.env.container_root.display(),
            tasks.len(),
        );

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                close(results_write_fd)?;

                self.assign_child_cgroup(child);
                let (killer_handle, cancel_kill) = Self::spawn_killer(child);

                let results_json = self.read_all_from_fd(results_read_fd);
                let _ = waitpid(child, None).map_err(Error::NixError)?;

                cancel_kill.store(true, Ordering::SeqCst);
                let _ = killer_handle.join();

                let results = Self::deserialize_results(&results_json)?;
                let _ = self.env.cleanup();
                Ok(results)
            }
            Ok(ForkResult::Child) => {
                let results_vec = match self.run_tasks_in_child(tasks) {
                    Ok(v) => v,
                    Err(e) => vec![TaskResult {
                        stdout: String::new(),
                        stderr: format!("setup failed: {e:?}"),
                        exit_code: 1,
                    }],
                };
                self.write_child_results(results_write_fd, &results_vec);
                std::process::exit(0);
            }
            Err(e) => Err(Error::GenericError(format!(
                "Fork failed in parent process: {e:?}"
            ))),
        }
    }

    fn assign_child_cgroup(&self, child: Pid) -> Option<CgroupHandle> {
        self.cgroups
            .assign_child(child, &self.env.container_root)
            .ok()
            .flatten()
    }

    fn spawn_killer(child: Pid) -> (std::thread::JoinHandle<()>, Arc<AtomicBool>) {
        let cancel_kill = Arc::new(AtomicBool::new(false));
        let cancel_kill_for_thread = cancel_kill.clone();
        let killer_handle = thread::spawn(move || {
            let timeout = Duration::from_secs(300);
            let start = std::time::Instant::now();
            while start.elapsed() < timeout {
                if cancel_kill_for_thread.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
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
        let _ = reader.read_to_string(&mut s);
        s
    }

    fn deserialize_results(json: &str) -> Result<Vec<TaskResult>> {
        serde_json::from_str(json).map_err(|e| {
            Error::GenericError(format!(
                "Failed to parse child results JSON: {e}. Raw: {}",
                json.chars().take(256).collect::<String>()
            ))
        })
    }

    fn write_child_results(&self, write_fd: OwnedFd, results: &Vec<TaskResult>) {
        let mut writer = unsafe { File::from_raw_fd(write_fd.into_raw_fd()) };
        let serialized = serde_json::to_string(results)
            .map_err(|e| Error::GenericError(format!("Failed to serialize results: {e}")))
            .unwrap_or_else(|e| {
                eprintln!("serialize error: {e:?}");
                String::from("[]")
            });
        use std::io::Write as _;
        let _ = writer.write_all(serialized.as_bytes());
        let _ = writer.flush();
    }

    fn run_tasks_in_child(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        self.env.initialize()?;
        let mut all_results: Vec<TaskResult> = Vec::with_capacity(tasks.len());
        for task in tasks.into_iter() {
            all_results.push(self.run_task(task)?);
        }
        Ok(all_results)
    }

    fn run_task(&self, task: Task) -> Result<TaskResult> {
        eprintln!("[faber:task] cmd='{}'", task.cmd);
        if let Some(files) = &task.files {
            if let Some(cwd) = &task.cwd {
                use std::collections::HashMap;
                let mut remapped: HashMap<String, String> = HashMap::new();
                for (k, v) in files.iter() {
                    let path = if k.starts_with('/') {
                        k.clone()
                    } else {
                        format!("{}/{}", cwd, k)
                    };
                    remapped.insert(path, v.clone());
                }
                self.env.write_files_to_workdir(&remapped)?;
            } else {
                self.env.write_files_to_workdir(files)?;
            }
        }

        let (task_stdout_read_fd, task_stdout_write_fd) = pipe()?;
        let (task_stderr_read_fd, task_stderr_write_fd) = pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                close(task_stdout_write_fd)?;
                close(task_stderr_write_fd)?;

                let mut task_stdout_reader =
                    unsafe { File::from_raw_fd(task_stdout_read_fd.into_raw_fd()) };
                let mut stdout_buf = String::new();
                let _ = task_stdout_reader.read_to_string(&mut stdout_buf);

                let mut task_stderr_reader =
                    unsafe { File::from_raw_fd(task_stderr_read_fd.into_raw_fd()) };
                let mut stderr_buf = String::new();
                let _ = task_stderr_reader.read_to_string(&mut stderr_buf);

                let status = waitpid(child, None).map_err(Error::NixError)?;
                let exit_code = match status {
                    WaitStatus::Exited(_, code) => code,
                    WaitStatus::Signaled(_, sig, _) => 128 + (sig as i32),
                    _ => 1,
                };

                Ok(TaskResult {
                    stdout: stdout_buf,
                    stderr: stderr_buf,
                    exit_code,
                })
            }
            Ok(ForkResult::Child) => {
                close(task_stdout_read_fd)?;
                close(task_stderr_read_fd)?;

                let mut stdout_fd = unsafe { ManuallyDrop::new(OwnedFd::from_raw_fd(1)) };
                let mut stderr_fd = unsafe { ManuallyDrop::new(OwnedFd::from_raw_fd(2)) };
                dup2(&task_stdout_write_fd, &mut stdout_fd)?;
                dup2(&task_stderr_write_fd, &mut stderr_fd)?;
                close(task_stdout_write_fd)?;
                close(task_stderr_write_fd)?;

                let desired_cwd = task
                    .cwd
                    .clone()
                    .unwrap_or_else(|| self.env.work_dir.clone());
                if !desired_cwd.is_empty() {
                    let _ = std::fs::create_dir_all(&desired_cwd);
                    std::env::set_current_dir(&desired_cwd).map_err(|e| {
                        Error::GenericError(format!("chdir to work_dir {desired_cwd} failed: {e}"))
                    })?;
                }

                let args = self.build_args(&task)?;
                let env = self.build_env(&task)?;

                let prog = CString::new(task.cmd.as_str()).unwrap();
                execve(&prog, &args, &env)
                    .map_err(|e| Error::GenericError(format!("execve failed: {e:?}")))?;
                unreachable!()
            }
            Err(e) => Ok(TaskResult {
                stdout: String::new(),
                stderr: format!("fork failed for task '{}': {e:?}", task.cmd),
                exit_code: 1,
            }),
        }
    }

    // === Only for the child process ===
    fn build_args(&self, task: &Task) -> Result<Vec<CString>> {
        let Some(args) = &task.args else {
            return Ok(vec![CString::new(task.cmd.as_str()).unwrap()]);
        };

        let mut result = vec![CString::new(task.cmd.as_str())?];
        for arg in args {
            result.push(CString::new(arg.to_owned())?);
        }

        Ok(result)
    }

    // === Only for the child process ===
    fn build_env(&self, task: &Task) -> Result<Vec<CString>> {
        let Some(env) = &task.env else {
            return Ok(vec![]);
        };

        let mut result = vec![];
        for (key, value) in env {
            result.push(CString::new(format!("{key}={value}"))?);
        }

        Ok(result)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        // Random container root path
        let id: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let container_root = PathBuf::from(format!("/tmp/faber/containers/{id}"));
        let flags = vec![
            nix::mount::MsFlags::MS_BIND,
            nix::mount::MsFlags::MS_REC,
            nix::mount::MsFlags::MS_RDONLY,
        ];
        let mounts = ["/bin", "/lib", "/usr", "/lib64", "/sbin"]
            .iter()
            .map(|s| crate::types::Mount {
                source: s.to_string(),
                target: s.to_string(),
                flags: flags.clone(),
                options: vec![],
                data: None,
            })
            .collect();
        let env =
            ContainerEnvironment::new(container_root, "faber".into(), mounts, "/faber".into());
        Self {
            env,
            cgroups: Cgroups::default(),
        }
    }
}
