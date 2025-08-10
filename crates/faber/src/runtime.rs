use nix::{
    sys::{
        signal::{Signal, kill},
        wait::waitpid,
    },
    unistd::{ForkResult, Pid, close, fork, pipe},
};
use rand::{Rng, distr::Alphanumeric};

use crate::{
    TaskResult,
    builder::RuntimeBuilder,
    cgroups::{CgroupHandle, Cgroups},
    environment::ContainerEnvironment,
    prelude::*,
    types::{RuntimeLimits, Task},
};

use std::process::{Command, Stdio};
use std::{
    collections::HashMap,
    ffi::CString,
    fs::File,
    io::Read,
    os::fd::{FromRawFd, IntoRawFd, OwnedFd},
    os::unix::process::ExitStatusExt,
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
    pub(crate) limits: RuntimeLimits,
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

                let cg_handle = self.cgroups.assign_child(child, &self.env.container_root)?;

                let timeout_secs = self.limits.kill_timeout_seconds.unwrap_or(10);
                let (killer_handle, cancel_kill) = Self::spawn_killer(child, timeout_secs);

                let results_json = self.read_all_from_fd(results_read_fd);
                let _ = waitpid(child, None).map_err(Error::NixError)?;

                cancel_kill.store(true, Ordering::SeqCst);
                let _ = killer_handle.join();

                if let Some(handle) = &cg_handle {
                    self.cgroups.cleanup_group(handle.path());
                }

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

    fn spawn_killer(
        child: Pid,
        timeout_secs: u64,
    ) -> (std::thread::JoinHandle<()>, Arc<AtomicBool>) {
        let cancel_kill = Arc::new(AtomicBool::new(false));
        let cancel_kill_for_thread = cancel_kill.clone();
        let killer_handle = thread::spawn(move || {
            let timeout = Duration::from_secs(timeout_secs);
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
        // Set up namespaces and root
        self.env.initialize()?;

        // Create an internal pipe to shuttle all results from PID 1 back to this parent
        let (read_fd, write_fd) = pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                // Parent of PID 1: close write end and read Vec<TaskResult> JSON
                close(write_fd)?;
                let json =
                    self.read_all_from_fd(unsafe { OwnedFd::from_raw_fd(read_fd.into_raw_fd()) });
                let _ = waitpid(child, None).map_err(Error::NixError)?;
                let results: Vec<TaskResult> = Self::deserialize_results(&json)?;
                Ok(results)
            }
            Ok(ForkResult::Child) => {
                // This becomes PID 1 in the new PID namespace
                close(read_fd)?;
                let results = self.run_tasks(tasks)?;
                self.write_child_results(
                    unsafe { OwnedFd::from_raw_fd(write_fd.into_raw_fd()) },
                    &results,
                );
                std::process::exit(0);
            }
            Err(e) => Err(Error::GenericError(format!(
                "failed to fork PID1 for task runner: {e:?}"
            ))),
        }
    }

    fn run_tasks(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        let mut all_results: Vec<TaskResult> = Vec::with_capacity(tasks.len());
        for task in tasks.into_iter() {
            eprintln!("[faber:task] cmd='{}'", task.cmd);
            if let Some(files) = &task.files {
                if let Some(cwd) = &task.cwd {
                    let mut remapped: HashMap<String, String> = HashMap::new();
                    for (k, v) in files.iter() {
                        let path = if k.starts_with('/') {
                            k.clone()
                        } else {
                            format!("{cwd}/{k}")
                        };
                        remapped.insert(path, v.clone());
                    }
                    self.env.write_files_to_workdir(&remapped)?;
                } else {
                    self.env.write_files_to_workdir(files)?;
                }
            }

            let mut cmd = Command::new(&task.cmd);
            if let Some(args) = &task.args {
                cmd.args(args);
            }
            if let Some(env) = &task.env {
                cmd.envs(env.iter());
            }

            if let Some(cwd) = &task.cwd {
                if !cwd.is_empty() {
                    cmd.current_dir(cwd);
                }
            } else if !self.env.work_dir.is_empty() {
                cmd.current_dir(&self.env.work_dir);
            }

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

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
            limits: RuntimeLimits::default(),
        }
    }
}
