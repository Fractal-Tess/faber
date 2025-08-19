use crate::prelude::*;
use nix::{
    sys::wait::waitpid,
    sys::signal::{kill, Signal},
    unistd::{Pid, close},
};
use std::{
    io::{PipeReader, PipeWriter, pipe},
    os::fd::RawFd,
    time::{Duration, Instant},
    thread,
};

/// Create a POSIX pipe, returning reader and writer ends.
///
/// Errors include OS-level failures to allocate the pipe.
pub fn mk_pipe() -> Result<(PipeReader, PipeWriter)> {
    pipe().map_err(|e| Error::ProcessManagement {
        operation: "create pipe".to_string(),
        pid: -1,
        details: format!("Failed to create pipe: {e}"),
    })
}

/// Close a raw file descriptor using `nix::unistd::close`.
pub fn close_fd(fd: RawFd) -> Result<()> {
    close(fd).map_err(|e| Error::ProcessManagement {
        operation: "close fd".to_string(),
        pid: -1,
        details: format!("Failed to close fd {fd}: {e}"),
    })?;
    Ok(())
}

/// Wait for a child process to exit and propagate errors.
pub fn wait_for_child(pid: Pid) -> Result<()> {
    waitpid(pid, None).map_err(|e| Error::ProcessManagement {
        operation: "wait for child".to_string(),
        pid: pid.as_raw(),
        details: format!("Failed to wait for child: {e}"),
    })?;

    Ok(())
}

/// Wait for a child process to exit with an optional timeout.
/// If timeout is reached, the child process is killed with SIGKILL.
pub fn wait_for_child_with_timeout(pid: Pid, timeout_seconds: Option<u64>) -> Result<bool> {
    if let Some(timeout) = timeout_seconds {
        let timeout_duration = Duration::from_secs(timeout);
        let start_time = Instant::now();
        
        // Spawn a monitoring thread
        let pid_clone = pid;
        let timeout_handle = thread::spawn(move || {
            thread::sleep(timeout_duration);
            let _ = kill(pid_clone, Signal::SIGKILL);
        });
        
        // Wait for the child to exit
        let wait_result = waitpid(pid, None);
        
        // Cancel the timeout thread if child exited before timeout
        drop(timeout_handle);
        
        match wait_result {
            Ok(_) => Ok(false), // Child exited normally
            Err(e) => {
                // Check if the child was killed due to timeout
                if start_time.elapsed() >= timeout_duration {
                    Ok(true) // Child was killed due to timeout
                } else {
                    Err(Error::ProcessManagement {
                        operation: "wait for child".to_string(),
                        pid: pid.as_raw(),
                        details: format!("Failed to wait for child: {e}"),
                    })
                }
            }
        }
    } else {
        // No timeout, use the original wait function
        wait_for_child(pid)?;
        Ok(false)
    }
}
