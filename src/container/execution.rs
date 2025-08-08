use std::collections::HashMap;
use std::os::fd::{IntoRawFd, RawFd};
use std::os::unix::prelude::FromRawFd;
use std::path::Path;

use nix::libc;
use nix::sched::{CloneFlags, unshare};
use nix::sys::wait::{WaitStatus, waitpid};
use nix::unistd::{ForkResult, chdir, chroot, close, execvpe, fork, pipe};
use tracing::info;

use super::errors::ContainerError;
use super::mounts;

/// Small wrapper around libc::dup2 to avoid OwnedFd API constraints.
///
/// This function duplicates a file descriptor, making `newfd` be a copy of `oldfd`.
/// If `newfd` was previously open, it is silently closed before being reused.
///
/// # Arguments
/// * `oldfd` - The file descriptor to duplicate
/// * `newfd` - The file descriptor number to use for the duplicate
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(nix::Error)` if the dup2 system call fails
fn dup2_raw(oldfd: RawFd, newfd: RawFd) -> Result<(), nix::Error> {
    let res = unsafe { libc::dup2(oldfd, newfd) };
    if res == -1 {
        Err(nix::Error::last())
    } else {
        Ok(())
    }
}

/// Container for the four file descriptors used for stdout/stderr pipes between processes.
///
/// This struct encapsulates the pipe file descriptors used for inter-process communication
/// during container execution. The parent process reads from the read ends while the child
/// process writes to the write ends.
struct ProcessPipes {
    /// Read end of stdout pipe (parent reads from this)
    stdout_r: RawFd,
    /// Write end of stdout pipe (child writes to this)
    stdout_w: RawFd,
    /// Read end of stderr pipe (parent reads from this)
    stderr_r: RawFd,
    /// Write end of stderr pipe (child writes to this)
    stderr_w: RawFd,
}

/// Creates and configures pipes for stdout and stderr communication between parent and child processes.
///
/// This function creates two pipes (one for stdout, one for stderr) that will be used to
/// capture the output from the isolated process. The file descriptors are converted to
/// raw FDs to avoid OwnedFd API constraints when crossing fork boundaries.
///
/// # Returns
/// * `Ok(ProcessPipes)` - Container with the four pipe file descriptors
/// * `Err(ContainerError::Pipe)` - If pipe creation fails
fn setup_pipes() -> Result<ProcessPipes, ContainerError> {
    let (stdout_r_fd, stdout_w_fd) = pipe().map_err(ContainerError::Pipe)?;
    let (stderr_r_fd, stderr_w_fd) = pipe().map_err(ContainerError::Pipe)?;

    // Convert to raw fds to avoid OwnedFd complexities across fork
    Ok(ProcessPipes {
        stdout_r: stdout_r_fd.into_raw_fd(),
        stdout_w: stdout_w_fd.into_raw_fd(),
        stderr_r: stderr_r_fd.into_raw_fd(),
        stderr_w: stderr_w_fd.into_raw_fd(),
    })
}

/// Handles the parent process logic: reads from pipes, waits for child completion, and returns results.
///
/// This function runs in the original parent process and is responsible for:
/// 1. Closing the write ends of the pipes (child will use these)
/// 2. Reading stdout and stderr from the pipes using separate threads
/// 3. Waiting for the child process to complete
/// 4. Converting the captured output to strings and determining the exit code
///
/// # Arguments
/// * `request_id` - Unique identifier for this execution request (for logging)
/// * `child_pid` - Process ID of the child process to wait for
/// * `pipes` - The pipe file descriptors for communication
///
/// # Returns
/// * `Ok((stdout, stderr, exit_code))` - Captured output and exit status
/// * `Err(ContainerError)` - If waiting for child or reading pipes fails
fn handle_parent_process(
    request_id: &str,
    child_pid: nix::unistd::Pid,
    pipes: ProcessPipes,
) -> Result<(String, String, i32), ContainerError> {
    // Parent: close write ends, read from read ends, wait for child
    let _ = close(pipes.stdout_w);
    let _ = close(pipes.stderr_w);

    let mut stdout_reader = unsafe { std::fs::File::from_raw_fd(pipes.stdout_r) };
    let mut stderr_reader = unsafe { std::fs::File::from_raw_fd(pipes.stderr_r) };

    let stdout_handle = std::thread::spawn(move || {
        let mut local = Vec::new();
        let _ = std::io::copy(&mut stdout_reader, &mut local);
        local
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut local = Vec::new();
        let _ = std::io::copy(&mut stderr_reader, &mut local);
        local
    });

    let status = waitpid(child_pid, None).map_err(|e| ContainerError::WaitPid {
        pid: child_pid.as_raw(),
        source: e,
    })?;
    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_handle.join().unwrap_or_default();

    let exit_code = match status {
        WaitStatus::Exited(_, code) => code,
        WaitStatus::Signaled(_, sig, _core) => 128 + sig as i32,
        _ => -1,
    };

    let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();
    info!(request_id = %request_id, exit_code = exit_code, stdout_bytes = stdout_bytes.len(), stderr_bytes = stderr_bytes.len(), "Isolated run completed");
    Ok((stdout, stderr, exit_code))
}

/// Creates Linux namespaces for process isolation.
///
/// This function creates several Linux namespaces to isolate the child process:
/// - CLONE_NEWNS: Mount namespace isolation
/// - CLONE_NEWPID: Process ID namespace isolation  
/// - CLONE_NEWUTS: Hostname and domain name isolation
/// - CLONE_NEWIPC: Inter-process communication isolation
/// - CLONE_NEWNET: Network namespace isolation
///
/// # Returns
/// * `Ok(())` - If namespace creation succeeds
/// * `Err(ContainerError::Unshare)` - If unshare system call fails
fn create_namespaces_and_mount_essentials(root: &Path) -> Result<(), ContainerError> {
    let flags = CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWPID
        | CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWIPC
        | CloneFlags::CLONE_NEWNET;
    unshare(flags).map_err(|e| ContainerError::Unshare { flags, source: e })?;

    // Mount essential virtual filesystems now that we're in a new mount namespace
    mounts::mount_essential_filesystems(root)?;
    Ok(())
}

/// Builds the argument vector (argv) for execvpe from command name and arguments.
///
/// This function creates a vector of null-terminated C strings suitable for passing
/// to execvpe. The first element is always the command name, followed by the arguments.
/// All strings are converted to CString to ensure they are null-terminated.
///
/// # Arguments
/// * `cmd` - The command to execute (becomes argv[0])
/// * `args` - Additional command-line arguments
///
/// # Returns
/// * `Ok(Vec<CString>)` - Vector of null-terminated argument strings
/// * `Err(ContainerError::CString)` - If any string contains null bytes
fn build_argv(cmd: &str, args: &[String]) -> Result<Vec<std::ffi::CString>, ContainerError> {
    let mut argv: Vec<std::ffi::CString> = Vec::with_capacity(args.len() + 1);
    argv.push(
        std::ffi::CString::new(cmd).map_err(|e| ContainerError::CString {
            value: cmd.to_string(),
            source: e,
        })?,
    );
    for a in args {
        argv.push(
            std::ffi::CString::new(a.as_str()).map_err(|e| ContainerError::CString {
                value: a.clone(),
                source: e,
            })?,
        );
    }
    Ok(argv)
}

/// Builds the environment vector (envp) for execvpe from environment variables.
///
/// This function creates a vector of null-terminated C strings in "KEY=VALUE" format
/// suitable for passing to execvpe. If no PATH variable is provided in the input,
/// a default PATH of "/usr/bin:/bin" is automatically added.
///
/// # Arguments
/// * `env` - HashMap of environment variable names to values
///
/// # Returns
/// * `Ok(Vec<CString>)` - Vector of null-terminated environment strings
/// * `Err(ContainerError::CString)` - If any string contains null bytes
fn build_envp(env: &HashMap<String, String>) -> Result<Vec<std::ffi::CString>, ContainerError> {
    let mut envp: Vec<std::ffi::CString> = Vec::with_capacity(env.len() + 1);
    let mut has_path = false;
    for (k, v) in env.iter() {
        if k == "PATH" {
            has_path = true;
        }
        let kv = format!("{}={}", k, v);
        envp.push(
            std::ffi::CString::new(kv.clone()).map_err(|e| ContainerError::CString {
                value: kv,
                source: e,
            })?,
        );
    }
    if !has_path {
        let kv = "PATH=/usr/bin:/bin".to_string();
        envp.push(std::ffi::CString::new(kv).unwrap());
    }
    Ok(envp)
}

/// Sets up stdio redirection by duplicating pipe file descriptors to stdout/stderr.
///
/// This function redirects the process's stdout and stderr to write to the provided pipes,
/// allowing the parent process to capture the output. After redirection, all pipe file
/// descriptors are closed since they're no longer needed.
///
/// # Arguments
/// * `pipes` - The pipe file descriptors to redirect to
///
/// # Returns
/// * `Ok(())` - If redirection succeeds
/// * `Err(ContainerError::Dup2)` - If dup2 system call fails
fn setup_stdio_redirection(pipes: &ProcessPipes) -> Result<(), ContainerError> {
    dup2_raw(pipes.stdout_w, libc::STDOUT_FILENO).map_err(|e| ContainerError::Dup2 {
        fd: pipes.stdout_w,
        target: libc::STDOUT_FILENO,
        source: e,
    })?;
    dup2_raw(pipes.stderr_w, libc::STDERR_FILENO).map_err(|e| ContainerError::Dup2 {
        fd: pipes.stderr_w,
        target: libc::STDERR_FILENO,
        source: e,
    })?;
    // Close fds we don't need anymore
    let _ = close(pipes.stdout_r);
    let _ = close(pipes.stderr_r);
    let _ = close(pipes.stdout_w);
    let _ = close(pipes.stderr_w);
    Ok(())
}

/// Sets up the chroot environment by changing root directory and working directory.
///
/// This function performs two critical steps for container isolation:
/// 1. chroot() - Changes the root directory to the container's filesystem
/// 2. chdir() - Changes the working directory to the specified work directory within the container
///
/// The work directory path is normalized by removing leading slashes since we're now
/// operating within the chroot environment.
///
/// # Arguments
/// * `root` - Path to the container's root directory (host filesystem)
/// * `work_dir_rel` - Relative path to the working directory within the container
///
/// # Returns
/// * `Ok(())` - If both chroot and chdir succeed
/// * `Err(ContainerError::Exec)` - If either system call fails
fn setup_chroot_environment(root: &Path, work_dir_rel: &str) -> Result<(), ContainerError> {
    chroot(root).map_err(|e| ContainerError::Exec {
        cmd: "chroot".into(),
        source: e,
    })?;
    chdir(Path::new(&format!(
        "/{}",
        work_dir_rel.trim_start_matches('/')
    )))
    .map_err(|e| ContainerError::Exec {
        cmd: "chdir".into(),
        source: e,
    })?;
    Ok(())
}

/// Executes the specified command with arguments and environment using execvpe.
///
/// This function performs the final step of process execution by calling execvpe,
/// which searches for the command in PATH and replaces the current process image.
/// This function should not return on success - the process image is replaced.
///
/// # Arguments
/// * `cmd` - Command name to execute
/// * `argv` - Argument vector (including command name as first element)
/// * `envp` - Environment vector in "KEY=VALUE" format
///
/// # Returns
/// * `Ok(())` - Should never return on success (process is replaced)
/// * `Err(ContainerError::Exec)` - If execvpe fails
/// * `Err(ContainerError::CString)` - If command name contains null bytes
fn execute_command(
    cmd: &str,
    argv: &[std::ffi::CString],
    envp: &[std::ffi::CString],
) -> Result<(), ContainerError> {
    let filename = std::ffi::CString::new(cmd).map_err(|e| ContainerError::CString {
        value: cmd.to_string(),
        source: e,
    })?;
    let argv_refs: Vec<&std::ffi::CStr> = argv.iter().map(|c| c.as_c_str()).collect();
    let envp_refs: Vec<&std::ffi::CStr> = envp.iter().map(|c| c.as_c_str()).collect();
    execvpe(&filename, &argv_refs, &envp_refs).map_err(|e| ContainerError::Exec {
        cmd: cmd.to_string(),
        source: e,
    })?;
    Ok(())
}

/// Handles the grandchild process: the final process that runs the user's command.
///
/// This function orchestrates the complete setup of the isolated execution environment:
/// 1. Redirects stdio to pipes for output capture
/// 2. Sets up chroot environment for filesystem isolation
/// 3. Builds command arguments and environment variables
/// 4. Executes the user's command (this should not return)
///
/// This process runs inside all the namespaces created earlier and represents the
/// actual isolated execution environment.
///
/// # Arguments
/// * `root` - Path to container root directory
/// * `work_dir_rel` - Working directory within the container
/// * `cmd` - Command to execute
/// * `args` - Command arguments
/// * `env` - Environment variables
/// * `pipes` - Pipe file descriptors for output capture
///
/// # Returns
/// * Should never return on success (process is replaced by exec)
/// * `Err(ContainerError)` - If any setup step fails
fn handle_grandchild_process(
    root: &Path,
    work_dir_rel: &str,
    cmd: &str,
    args: &[String],
    env: &HashMap<String, String>,
    pipes: ProcessPipes,
) -> Result<(), ContainerError> {
    // Grandchild: redirect stdio to pipes
    setup_stdio_redirection(&pipes)?;

    // chroot and chdir
    setup_chroot_environment(root, work_dir_rel)?;

    // Build argv and envp
    let argv = build_argv(cmd, args)?;
    let envp = build_envp(env)?;

    // exec (search PATH)
    execute_command(cmd, &argv, &envp)?;
    unreachable!()
}

/// Handles the child manager process within the new PID namespace.
///
/// This process serves as an intermediate manager that runs as PID 1 within the new
/// PID namespace. Its responsibilities are:
/// 1. Close the write ends of pipes (grandchild will use these)
/// 2. Wait for the grandchild process to complete
/// 3. Exit with the same code as the grandchild
///
/// This process is necessary because when creating a new PID namespace, the first
/// process to enter it becomes PID 1 and must handle child reaping.
///
/// # Arguments
/// * `child_pid` - Process ID of the grandchild process to wait for
/// * `pipes` - Pipe file descriptors (write ends will be closed)
///
/// # Returns
/// * Should never return (calls libc::_exit)
/// * `Err(ContainerError::WaitPid)` - If waiting for grandchild fails
fn handle_child_manager_process(
    child_pid: nix::unistd::Pid,
    pipes: ProcessPipes,
) -> Result<(), ContainerError> {
    // Manager inside new PID namespace: close write ends, wait for grandchild
    let _ = close(pipes.stdout_w);
    let _ = close(pipes.stderr_w);
    let status = waitpid(child_pid, None).map_err(|e| ContainerError::WaitPid {
        pid: child_pid.as_raw(),
        source: e,
    })?;
    let code = match status {
        WaitStatus::Exited(_, code) => code,
        WaitStatus::Signaled(_, sig, _core) => 128 + sig as i32,
        _ => -1,
    };
    unsafe { libc::_exit(code) };
}

/// Runs a command in an isolated container environment using Linux namespaces and chroot.
///
/// This is the main entry point for container execution. It creates a highly isolated
/// environment using multiple Linux namespaces and chroot, then executes the specified
/// command within that environment. The function uses a double-fork pattern:
///
/// 1. **First fork**: Creates namespaces and serves as the parent for result collection
/// 2. **Second fork**: Creates a manager process (PID 1 in new PID namespace)  
/// 3. **Grandchild**: The actual isolated process that runs the user's command
///
/// The isolation includes:
/// - Filesystem isolation via chroot
/// - Process isolation via PID namespace
/// - Network isolation via network namespace
/// - IPC isolation via IPC namespace
/// - Hostname isolation via UTS namespace
/// - Mount isolation via mount namespace
///
/// # Arguments
/// * `request_id` - Unique identifier for this execution (used for logging)
/// * `root` - Path to the container's root filesystem directory
/// * `work_dir_rel` - Working directory path relative to the container root
/// * `cmd` - Command name to execute (will be searched in PATH)
/// * `args` - Command-line arguments to pass to the command
/// * `env` - Environment variables to set for the command
///
/// # Returns
/// * `Ok((stdout, stderr, exit_code))` - Captured output and exit status
/// * `Err(ContainerError)` - If any step of the isolation or execution fails
///
/// # Examples
/// ```rust
/// let result = run_isolated_blocking(
///     "req-123",
///     Path::new("/tmp/container-root"),
///     "work",
///     "echo",
///     &["Hello, World!".to_string()],
///     &HashMap::new(),
/// )?;
/// println!("Output: {}, Exit: {}", result.0, result.2);
/// ```
pub fn run_isolated_blocking(
    request_id: &str,
    root: &Path,
    work_dir_rel: &str,
    cmd: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> Result<(String, String, i32), ContainerError> {
    info!(request_id = %request_id, cmd = %cmd, "Starting isolated run");

    let pipes = setup_pipes()?;

    match unsafe { fork().map_err(ContainerError::Fork)? } {
        ForkResult::Parent { child } => handle_parent_process(request_id, child, pipes),
        ForkResult::Child => {
            // Child: create namespaces; for CLONE_NEWPID, the next fork will enter it
            create_namespaces_and_mount_essentials(root)?;

            match unsafe { fork().map_err(ContainerError::Fork)? } {
                ForkResult::Parent { child } => {
                    handle_child_manager_process(child, pipes)?;
                    unreachable!()
                }
                ForkResult::Child => {
                    handle_grandchild_process(root, work_dir_rel, cmd, args, env, pipes)?;
                    unreachable!()
                }
            }
        }
    }
}
