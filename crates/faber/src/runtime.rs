use nix::{
    errno::Errno,
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::{CloneFlags, unshare},
    sys::{
        signal::{Signal, kill},
        stat::{Mode, SFlag, makedev, mknod},
        wait::{WaitStatus, waitpid},
    },
    unistd::{ForkResult, close, dup2, execve, fork, pipe, pivot_root, sethostname},
};
use rand::{Rng, distr::Alphanumeric};

use crate::{
    TaskResult,
    builder::RuntimeBuilder,
    prelude::*,
    types::{CgroupConfig, Mount, Task},
};

use std::{
    ffi::CString,
    fs::File,
    io::Read,
    mem::ManuallyDrop,
    os::fd::{FromRawFd, IntoRawFd, OwnedFd},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Debug)]
pub struct Runtime {
    pub(crate) container_root: PathBuf,
    pub(crate) hostname: String,
    pub(crate) mounts: Vec<Mount>,
    pub(crate) cgroup: Option<CgroupConfig>,
    pub(crate) work_dir: String,
}

impl Runtime {
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    pub fn run(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Create a dedicated pipe for structured results from the child
        let (results_read_fd, results_write_fd) = pipe()?;
        eprintln!(
            "[faber:run] container_root={}, mounts={}, work_dir='{}', tasks={}",
            self.container_root.display(),
            self.mounts.len(),
            self.work_dir,
            tasks.len()
        );

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                // Parent: close the write end so reads can see EOF
                close(results_write_fd)?;

                // Create and assign cgroup for child (best-effort)
                let cgroup_path = self.setup_cgroup_for_child(child).ok().flatten();

                // Killer thread: if child doesn't exit in 10s, send SIGKILL
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

                // Read all structured results from the child
                let mut results_reader =
                    unsafe { File::from_raw_fd(results_read_fd.into_raw_fd()) };
                let mut results_json = String::new();
                let _ = results_reader.read_to_string(&mut results_json);

                let status = waitpid(child, None).map_err(Error::NixError)?;
                let _exit_code = match status {
                    WaitStatus::Exited(_, code) => code,
                    WaitStatus::Signaled(_, sig, _) => 128 + (sig as i32),
                    _ => 1,
                };

                // Cancel killer thread and join to avoid PID reuse races
                cancel_kill.store(true, Ordering::SeqCst);
                let _ = killer_handle.join();

                // Cleanup the cgroup if we created one
                if let Some(cg) = cgroup_path {
                    let _ = std::fs::remove_dir(cg);
                }

                // Deserialize results vector; include context on error
                let results: Vec<TaskResult> =
                    serde_json::from_str(&results_json).map_err(|e| {
                        Error::GenericError(format!(
                            "Failed to parse child results JSON: {e}. Raw: {}",
                            results_json.chars().take(256).collect::<String>()
                        ))
                    })?;

                // Clean up the container root folder
                self.clean_root()?;

                Ok(results)
            }
            Ok(ForkResult::Child) => {
                // Run setup and tasks; always write a JSON results payload back
                let results_vec = match self.child_manager(tasks) {
                    Ok(v) => v,
                    Err(e) => {
                        // Represent setup failure as a single TaskResult for API stability
                        vec![TaskResult {
                            stdout: String::new(),
                            stderr: format!("setup failed: {e:?}"),
                            exit_code: 1,
                        }]
                    }
                };
                // Safety: we own this fd in the child
                let mut results_writer =
                    unsafe { File::from_raw_fd(results_write_fd.into_raw_fd()) };
                let serialized = serde_json::to_string(&results_vec)
                    .map_err(|e| Error::GenericError(format!("Failed to serialize results: {e}")))
                    .unwrap_or_else(|e| {
                        // Fallback minimal JSON on unexpected serialization failure
                        eprintln!("serialize error: {e:?}");
                        String::from("[]")
                    });
                use std::io::Write as _;
                let _ = results_writer.write_all(serialized.as_bytes());
                let _ = results_writer.flush();
                std::process::exit(0);
            }

            Err(e) => Err(Error::GenericError(format!(
                "Fork failed in parent process: {e:?}"
            ))),
        }
    }

    fn child_manager(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // Proceed with isolation and setup in the top-level child
        self.initialize_container_root()?;

        // Sequentially execute tasks inside the namespace, capturing outputs per task
        let mut all_results: Vec<TaskResult> = Vec::with_capacity(tasks.len());

        for task in tasks.into_iter() {
            eprintln!("[faber:task] cmd='{}'", task.cmd);
            // Materialize any provided files for this task relative to its cwd or default work_dir
            if let Some(files) = &task.files {
                let base_dir = task.cwd.clone().unwrap_or_else(|| self.work_dir.clone());
                if !base_dir.is_empty() {
                    std::fs::create_dir_all(&base_dir)?;
                }
                for (rel_path, contents) in files {
                    let target_path = if rel_path.starts_with('/') {
                        // Absolute within container
                        rel_path.clone()
                    } else if base_dir.is_empty() {
                        rel_path.clone()
                    } else {
                        format!("{base_dir}/{rel_path}")
                    };
                    if let Some(parent) = Path::new(&target_path).parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&target_path, contents).map_err(|e| {
                        Error::GenericError(format!("failed to write task file {target_path}: {e}"))
                    })?;
                    eprintln!(
                        "[faber:task] wrote file {} ({} bytes)",
                        target_path,
                        contents.len()
                    );
                }
            }
            // Create pipes to capture this task's stdout/stderr
            let (task_stdout_read_fd, task_stdout_write_fd) = pipe()?;
            let (task_stderr_read_fd, task_stderr_write_fd) = pipe()?;

            match unsafe { fork() } {
                Ok(ForkResult::Parent { child, .. }) => {
                    // Parent (top-level child): close write ends and read outputs
                    close(task_stdout_write_fd)?;
                    close(task_stderr_write_fd)?;

                    // Read stdout
                    let mut task_stdout_reader =
                        unsafe { File::from_raw_fd(task_stdout_read_fd.into_raw_fd()) };
                    let mut stdout_buf = String::new();
                    let _ = task_stdout_reader.read_to_string(&mut stdout_buf);

                    // Read stderr
                    let mut task_stderr_reader =
                        unsafe { File::from_raw_fd(task_stderr_read_fd.into_raw_fd()) };
                    let mut stderr_buf = String::new();
                    let _ = task_stderr_reader.read_to_string(&mut stderr_buf);

                    // Wait for the grandchild to finish
                    let status = waitpid(child, None).map_err(Error::NixError)?;
                    let exit_code = match status {
                        WaitStatus::Exited(_, code) => code,
                        WaitStatus::Signaled(_, sig, _) => 128 + (sig as i32),
                        _ => 1,
                    };

                    all_results.push(TaskResult {
                        stdout: stdout_buf,
                        stderr: stderr_buf,
                        exit_code,
                    });
                }
                Ok(ForkResult::Child) => {
                    // Grandchild: set up stdout/stderr redirection to per-task pipes
                    // Close read ends
                    close(task_stdout_read_fd)?;
                    close(task_stderr_read_fd)?;

                    // Duplicate write ends onto STDOUT/STDERR without libc
                    let mut stdout_fd = unsafe { ManuallyDrop::new(OwnedFd::from_raw_fd(1)) };
                    let mut stderr_fd = unsafe { ManuallyDrop::new(OwnedFd::from_raw_fd(2)) };
                    dup2(&task_stdout_write_fd, &mut stdout_fd)?;
                    dup2(&task_stderr_write_fd, &mut stderr_fd)?;
                    // Close the original write fds; stdout/stderr now refer to the pipes
                    close(task_stdout_write_fd)?;
                    close(task_stderr_write_fd)?;

                    // Change to task-specific cwd or configured work_dir
                    let desired_cwd = task.cwd.clone().unwrap_or_else(|| self.work_dir.clone());
                    if !desired_cwd.is_empty() {
                        let _ = std::fs::create_dir_all(&desired_cwd);
                        std::env::set_current_dir(&desired_cwd).map_err(|e| {
                            Error::GenericError(format!(
                                "chdir to work_dir {desired_cwd} failed: {e}"
                            ))
                        })?;
                    }

                    // Prepare the arguments and env for execve
                    let args = self.build_args(&task)?;
                    let env = self.build_env(&task)?;

                    // Resolve candidate executable paths
                    let prog = CString::new(task.cmd.as_str()).unwrap();
                    execve(&prog, &args, &env)
                        .map_err(|e| Error::GenericError(format!("execve failed: {e:?}")))?;
                    unreachable!()
                }
                Err(e) => {
                    all_results.push(TaskResult {
                        stdout: String::new(),
                        stderr: format!("fork failed for task '{}': {e:?}", task.cmd),
                        exit_code: 1,
                    });
                    continue;
                }
            }
        }

        Ok(all_results)
    }

    fn print_entries(&self, path: &Path) -> Result<()> {
        let stat = std::fs::read_dir(path)?;
        for entry in stat {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = entry.metadata().unwrap();
            eprintln!("path: {path:?}");
        }
        Ok(())
    }

    fn initialize_container_root(&self) -> Result<()> {
        use crate::environment::ContainerEnvironment;
        let env = ContainerEnvironment::new(
            self.container_root.clone(),
            self.hostname.clone(),
            self.mounts.clone(),
            self.work_dir.clone(),
        );

        env.unshare()?;
        env.bind_mounts()?;
        env.print_entries(&self.container_root)?;
        env.create_proc_sys()?;
        env.print_entries(&self.container_root)?;
        env.create_tmp()?;
        env.print_entries(&self.container_root)?;
        env.create_work_dir()?;
        env.print_entries(&self.container_root)?;
        env.create_devices()?;
        env.print_entries(&self.container_root)?;
        env.set_hostname()?;
        env.pivot_root()?;

        eprintln!("[faber:child] pivot_root complete; cwd=/");
        Ok(())
    }

    /// === Only for the parent process ===
    fn clean_root(&self) -> Result<()> {
        std::fs::remove_dir_all(&self.container_root)
            .map_err(|e| Error::GenericError(format!("failed to remove container root: {e}")))?;
        Ok(())
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

    /// === Only for the child process ===
    fn unshare(&self) -> Result<()> {
        let flags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET;

        unshare(flags).map_err(|_| Error::UnshareFailed)?;

        Ok(())
    }

    // === Only for the child process ===
    fn set_hostname(&self) -> Result<()> {
        sethostname(self.hostname.as_str()).map_err(Error::NixError)?;
        Ok(())
    }

    /// Create a cgroup v2 for the child process and move it there. Best-effort; returns the path created.
    fn setup_cgroup_for_child(&self, child: nix::unistd::Pid) -> Result<Option<PathBuf>> {
        // Ensure unified cgroup v2 hierarchy is mounted at /sys/fs/cgroup
        let cgroup_root = Path::new("/sys/fs/cgroup");
        if !cgroup_root.exists() {
            return Ok(None);
        }

        // Base path for faber-managed groups
        let faber_base = cgroup_root.join("faber");
        let _ = std::fs::create_dir_all(&faber_base);

        // Attempt to enable common controllers on the base (ignore errors)
        let subtree_control = faber_base.join("cgroup.subtree_control");
        let _ = std::fs::write(&subtree_control, b"+pids +cpu +memory");

        // Derive group name from container root folder name or fallback to pid
        let group_name = self
            .container_root
            .file_name()
            .map(|os| os.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("pid-{child}"));
        let group_path = faber_base.join(group_name);
        std::fs::create_dir_all(&group_path)?;

        // Optional limits if configured
        // (format per cgroup v2 files: pids.max, memory.max, cpu.max)
        if let Some(cfg) = &self.cgroup {
            if let Some(v) = &cfg.pids_max {
                let _ = std::fs::write(group_path.join("pids.max"), v);
            }
            if let Some(v) = &cfg.memory_max {
                let _ = std::fs::write(group_path.join("memory.max"), v);
            }
            if let Some(v) = &cfg.cpu_max {
                let _ = std::fs::write(group_path.join("cpu.max"), v);
            }
        }

        // Move the child into the cgroup
        let procs_file = group_path.join("cgroup.procs");
        std::fs::write(&procs_file, child.as_raw().to_string())?;

        Ok(Some(group_path))
    }

    /// === Only for the child process ===
    fn create_tmp(&self) -> Result<()> {
        let tmp_path = format!("{}/tmp", self.container_root.display());
        // Ensure {container_root}/tmp exists and is writable with the sticky bit
        std::fs::create_dir_all(&tmp_path)?;
        mount(
            Some("tmpfs"),
            tmp_path.as_str(),
            Some("tmpfs"),
            MsFlags::empty(),
            Some("size=128M,mode=1777"),
        )
        .map_err(Error::NixError)?;
        Ok(())
    }

    /// === Only for the child process ===
    fn create_work_dir(&self) -> Result<()> {
        let work_dir = format!("{}/{}", self.container_root.display(), self.work_dir);
        std::fs::create_dir_all(&work_dir)?;
        Ok(())
    }

    /// === Only for the child process ===
    fn create_proc_sys(&self) -> Result<()> {
        // Mount procfs on /proc
        let proc_source = Some("proc");
        let proc_path = format!("{}/proc", self.container_root.display());
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        // Create /proc directory
        std::fs::create_dir_all(&proc_path)?;
        eprintln!("[faber:mount] mounting proc -> {}", proc_path);

        // Mount procfs on /proc
        mount(
            proc_source,
            proc_path.as_str(),
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        // Mount sysfs on /sys
        let sys_source = Some("sysfs");
        let sys_target = format!("{}/sys", self.container_root.display());
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        // Create /sys directory
        std::fs::create_dir_all(&sys_target)?;
        eprintln!("[faber:mount] mounting sysfs -> {}", sys_target);

        // Mount sysfs on /sys
        mount(
            sys_source,
            sys_target.as_str(),
            Some(sys_fstype),
            sys_flags,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        Ok(())
    }

    /// === Only for the child process ===
    fn create_devices(&self) -> Result<()> {
        let flags = SFlag::S_IFCHR;
        let mode = Mode::S_IRUSR
            | Mode::S_IWUSR
            | Mode::S_IRGRP
            | Mode::S_IWGRP
            | Mode::S_IROTH
            | Mode::S_IWOTH;

        // Ensure /dev directory exists
        let dev_path = format!("{}/dev", self.container_root.display());
        std::fs::create_dir_all(&dev_path)?;

        // Create essential character devices

        // Null device
        let device_path = format!("{dev_path}/null");
        let device_id = makedev(1, 3);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // Zero device
        let device_path = format!("{dev_path}/zero");
        let device_id = makedev(1, 5);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // Full device
        let device_path = format!("{dev_path}/full");
        let device_id = makedev(1, 7);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // Random device
        let device_path = format!("{dev_path}/random");
        let device_id = makedev(1, 8);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // Urandom device
        let device_path = format!("{dev_path}/urandom");
        let device_id = makedev(1, 9);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        Ok(())
    }

    /// === Only for the child process ===
    fn bind_mounts(&self) -> Result<()> {
        // Make mount propagation private so mounts don't propagate back to host
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        // Bind mount to the container root
        for m in &self.mounts {
            // Skip mounts whose source does not exist to avoid ENOENT
            if !Path::new(&m.source).exists() {
                eprintln!("skipping mount {}: source does not exist", m.source);
                continue;
            }
            let target = format!(
                "{}/{}",
                self.container_root.display(),
                // TODO: Fix this unwrap
                m.target.strip_prefix("/").unwrap().to_owned()
            );
            let flags = m
                .flags
                .iter()
                .fold(MsFlags::empty(), |acc, flag| acc | *flag);

            // Create the target directory if it doesn't exist
            std::fs::create_dir_all(&target)?;

            // Mount the source to the target
            match mount(
                Some(m.source.as_str()),
                target.as_str(),
                None::<&str>,
                flags,
                m.data.as_deref(),
            ) {
                Ok(_) => {
                    eprintln!("mounted {}: {}", m.source, target);
                }
                Err(e) => {
                    eprintln!("failed to mount {}: {e:?}", m.source);
                }
            }
        }
        Ok(())
    }

    /// === Only for the child process ===
    fn pivot_root(&self) -> Result<()> {
        let new_root = self.container_root.clone();
        let old_root = format!("{}/oldroot", self.container_root.display());

        // Ensure required directories exist
        std::fs::create_dir_all(&new_root)
            .map_err(|e| Error::GenericError(format!("failed to create new_root dir: {e}")))?;
        std::fs::create_dir_all(&old_root)
            .map_err(|e| Error::GenericError(format!("failed to create old_root dir: {e}")))?;

        // Remount the new root as bind mount
        eprintln!(
            "[faber:root] remount new_root {} as MS_BIND|MS_REC",
            new_root.display()
        );
        mount(
            Some(new_root.to_str().unwrap()),
            new_root.to_str().unwrap(),
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        // Switch root
        eprintln!(
            "[faber:root] pivot_root new_root={} old_root={}",
            new_root.display(),
            old_root
        );
        pivot_root(new_root.to_str().unwrap(), old_root.as_str()).map_err(Error::NixError)?;

        // Change directory to the new root
        std::env::set_current_dir("/")
            .map_err(|e| Error::GenericError(format!("chdir to new root failed: {e}")))?;

        // Unmount and remove the old root (now mounted at /oldroot)
        eprintln!("[faber:root] umount oldroot");
        umount2("/oldroot", MntFlags::MNT_DETACH).map_err(Error::NixError)?;
        let _ = std::fs::remove_dir_all("/oldroot");

        Ok(())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        // Default bind mounts for core system directories
        let flags = vec![MsFlags::MS_BIND, MsFlags::MS_REC, MsFlags::MS_RDONLY];
        let mounts: Vec<Mount> = ["/bin", "/lib", "/usr", "/lib64"]
            .iter()
            .map(|s| Mount {
                source: s.to_string(),
                target: s.to_string(),
                flags: flags.clone(),
                options: vec![],
                data: None,
            })
            .collect();

        // Random container root path
        let id: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let container_root = PathBuf::from(format!("/tmp/faber/containers/{id}"));

        Self {
            container_root,
            hostname: "faber".into(),
            mounts,
            cgroup: None,
            work_dir: "/faber".into(),
        }
    }
}
