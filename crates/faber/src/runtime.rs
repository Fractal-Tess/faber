use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::{CloneFlags, unshare},
    sys::{
        stat::{Mode, SFlag, makedev, mknod},
        wait::{WaitStatus, waitpid},
    },
    unistd::{ForkResult, close, dup2, execve, fork, pipe, pivot_root},
};

use crate::runtime_builder::RuntimeBuilder;
use crate::{
    TaskResult,
    prelude::*,
    types::{CgroupConfig, Mount, Task},
};
use nix::sys::signal::{Signal, kill};
use std::os::unix::io::{FromRawFd, IntoRawFd, OwnedFd};
use std::path::Path;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;
use std::{ffi::CString, fs::File};
use std::{io::Read, mem::ManuallyDrop};

pub struct Runtime {
    container_root: PathBuf,
    mounts: Vec<Mount>,
    cgroup: Option<CgroupConfig>,
    work_dir: String,
}

impl Runtime {
    pub fn new(container_root: String, mounts: Vec<Mount>) -> Self {
        RuntimeBuilder::new(container_root)
            .with_mounts(mounts)
            .build()
    }

    pub fn builder(container_root: String) -> RuntimeBuilder {
        RuntimeBuilder::new(container_root)
    }

    pub(crate) fn from_builder_parts(
        container_root: PathBuf,
        mounts: Vec<Mount>,
        cgroup: Option<CgroupConfig>,
        work_dir: String,
    ) -> Self {
        Self {
            container_root,
            mounts,
            cgroup,
            work_dir,
        }
    }

    pub fn run(&self, tasks: Vec<Task>) -> Result<TaskResult> {
        // Create pipes for capturing child's stdout and stderr
        // Since this is not in the match block, it is created for both parent and child
        let (stdout_read_fd, stdout_write_fd) = pipe()?;
        let (stderr_read_fd, stderr_write_fd) = pipe()?;

        match unsafe { fork() } {
            Ok(ForkResult::Parent { child, .. }) => {
                // Parent: close the write ends so reads can see EOF
                close(stdout_write_fd)?;
                close(stderr_write_fd)?;

                // Create and assign cgroup for child (best-effort)
                let cgroup_path = self.setup_cgroup_for_child(child).ok().flatten();

                // Spawn readers for stdout and stderr that return full strings
                let stdout_handle = thread::spawn(move || {
                    // Safety: we own this fd in the parent
                    let mut file = unsafe { File::from_raw_fd(stdout_read_fd.into_raw_fd()) };
                    let mut buf = String::new();
                    let _ = file.read_to_string(&mut buf);
                    buf
                });

                let stderr_handle = thread::spawn(move || {
                    // Safety: we own this fd in the parent
                    let mut file = unsafe { File::from_raw_fd(stderr_read_fd.into_raw_fd()) };
                    let mut buf = String::new();
                    let _ = file.read_to_string(&mut buf);
                    buf
                });

                // Killer thread: if child doesn't exit in 10s, send SIGKILL
                let cancel_kill = Arc::new(AtomicBool::new(false));
                let cancel_kill_for_thread = cancel_kill.clone();
                let killer_handle = thread::spawn(move || {
                    let timeout = Duration::from_secs(10);
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

                let status = waitpid(child, None).map_err(Error::NixError)?;
                let exit_code = match status {
                    WaitStatus::Exited(_, code) => code,
                    WaitStatus::Signaled(_, sig, _) => 128 + (sig as i32),
                    _ => 1,
                };

                // Cancel killer thread and join to avoid PID reuse races
                cancel_kill.store(true, Ordering::SeqCst);
                let _ = killer_handle.join();

                let stdout = stdout_handle.join().unwrap_or_default();
                let stderr = stderr_handle.join().unwrap_or_default();

                // Cleanup the cgroup if we created one
                if let Some(cg) = cgroup_path {
                    let _ = std::fs::remove_dir(cg);
                }

                self.clean_root()?;

                Ok(TaskResult {
                    stdout,
                    stderr,
                    exit_code,
                })
            }
            Ok(ForkResult::Child) => {
                match self.child_process(
                    tasks.first().unwrap().clone(),
                    (stdout_read_fd, stdout_write_fd),
                    (stderr_read_fd, stderr_write_fd),
                ) {
                    Ok(()) => {
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("child_process failed before execve: {e:?}");
                        std::process::exit(1);
                    }
                }
            }

            Err(e) => Err(Error::GenericError(format!(
                "Fork failed in parent process: {e:?}"
            ))),
        }
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

    fn child_process(
        &self,
        task: Task,
        stdout_pipe: (OwnedFd, OwnedFd),
        stderr_pipe: (OwnedFd, OwnedFd),
    ) -> Result<()> {
        // Redirect child's stdout/stderr to pipe write ends as early as possible
        let (stdout_read_fd, stdout_write_fd) = stdout_pipe;
        let (stderr_read_fd, stderr_write_fd) = stderr_pipe;

        // Close read ends in child; we don't need them in the child
        close(stdout_read_fd)?;
        close(stderr_read_fd)?;

        // Duplicate write ends onto STDOUT/STDERR without libc
        let mut stdout_fd = unsafe { ManuallyDrop::new(OwnedFd::from_raw_fd(1)) };
        let mut stderr_fd = unsafe { ManuallyDrop::new(OwnedFd::from_raw_fd(2)) };
        dup2(&stdout_write_fd, &mut stdout_fd)?;
        dup2(&stderr_write_fd, &mut stderr_fd)?;
        // Close the original write fds; stdout/stderr now refer to the pipes
        close(stdout_write_fd)?;
        close(stderr_write_fd)?;

        // Proceed with isolation and setup
        self.unshare()?;
        self.bind_mounts()?;
        self.create_proc_sys()?;
        self.create_devices()?;
        self.pivot_root()?;

        // Change to task-specific cwd or configured work_dir
        let desired_cwd = task.cwd.clone().unwrap_or_else(|| self.work_dir.clone());
        if !desired_cwd.is_empty() {
            let _ = std::fs::create_dir_all(&desired_cwd);
            std::env::set_current_dir(&desired_cwd).map_err(|e| {
                Error::GenericError(format!("chdir to work_dir {desired_cwd} failed: {e}"))
            })?;
        }

        // Prepare the arguments for execve
        let prog = CString::new(task.cmd.as_str()).unwrap();
        let args = self.build_args(&task)?;
        let env = self.build_env(&task)?;

        match execve(&prog, &args, &env) {
            Ok(_) => unreachable!(),
            Err(e) => {
                eprintln!("execve failed: {e:?}");
                std::process::exit(1);
            }
        }
    }

    /// === Only for the parent process ===
    fn clean_root(&self) -> Result<()> {
        if self.container_root.exists() {
            // Best-effort: unmount proc and sys (may have propagated if mounts weren't private)
            let proc_path = format!("{}/proc", self.container_root.display());
            let sys_path = format!("{}/sys", self.container_root.display());
            let _ = umount2(proc_path.as_str(), MntFlags::MNT_DETACH);
            let _ = umount2(sys_path.as_str(), MntFlags::MNT_DETACH);

            // Best-effort: unmount any bind mounts we created under the container root
            for m in &self.mounts {
                let target = format!(
                    "{}/{}",
                    self.container_root.display(),
                    m.target.strip_prefix("/").unwrap().to_owned()
                );
                let _ = umount2(target.as_str(), MntFlags::MNT_DETACH);
            }

            std::fs::remove_dir_all(&self.container_root)?
        }

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

        // Make mount propagation private so mounts do not propagate back to the host namespace
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(Error::NixError)?;
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
        for m in &self.mounts {
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
            mount(
                Some(m.source.as_str()),
                target.as_str(),
                None::<&str>,
                flags,
                m.data.as_deref(),
            )
            .map_err(Error::NixError)?;
        }
        Ok(())
    }

    /// === Only for the child process ===
    fn pivot_root(&self) -> Result<()> {
        let new_root = self.container_root.clone();
        let old_root = format!("{}/oldroot", self.container_root.display());

        // Remount the new root as bind mount
        mount(
            Some(new_root.to_str().unwrap()),
            new_root.to_str().unwrap(),
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        // Switch root
        pivot_root(new_root.to_str().unwrap(), old_root.as_str()).map_err(Error::NixError)?;

        // Change directory to the new root
        std::env::set_current_dir("/")
            .map_err(|e| Error::GenericError(format!("chdir to new root failed: {e}")))?;

        // Unmount and remove the old root (now mounted at /oldroot)
        umount2("/oldroot", MntFlags::MNT_DETACH).map_err(Error::NixError)?;
        let _ = std::fs::remove_dir_all("/oldroot");

        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.clean_root();
    }
}
