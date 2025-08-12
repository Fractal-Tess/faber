use crate::prelude::*;
use nix::{sys::wait::waitpid, unistd::Pid};
use std::{
    io::{PipeReader, PipeWriter, pipe},
    os::fd::RawFd,
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

/// Close a raw file descriptor.
///
/// Currently a no-op placeholder; kept to centralize FD lifecycle and allow
/// future platform-specific handling.
pub fn close_fd(fd: RawFd) -> Result<()> {
    let _ = fd;

    Ok(())
}

/// Wait for a child process to exit.
pub fn wait_for_child(pid: Pid) -> Result<()> {
    let _ = waitpid(pid, None).map_err(|e| Error::ProcessManagement {
        operation: "wait for child".to_string(),
        pid: pid.as_raw(),
        details: format!("Failed to wait for child: {e}"),
    });

    Ok(())
}
