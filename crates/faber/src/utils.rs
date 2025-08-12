use crate::prelude::*;
use nix::{
    sys::wait::waitpid,
    unistd::{Pid, close},
};
use std::{
    io::{PipeReader, PipeWriter, pipe},
    os::fd::RawFd,
};
use tracing::debug;

/// Create a POSIX pipe, returning reader and writer ends.
///
/// Errors include OS-level failures to allocate the pipe.
pub fn mk_pipe() -> Result<(PipeReader, PipeWriter)> {
    debug!("utils::mk_pipe");
    pipe().map_err(|e| Error::ProcessManagement {
        operation: "create pipe".to_string(),
        pid: -1,
        details: format!("Failed to create pipe: {e}"),
    })
}

/// Close a raw file descriptor using `nix::unistd::close`.
pub fn close_fd(fd: RawFd) -> Result<()> {
    debug!(fd, "utils::close_fd");
    close(fd).map_err(|e| Error::ProcessManagement {
        operation: "close fd".to_string(),
        pid: -1,
        details: format!("Failed to close fd {fd}: {e}"),
    })?;
    Ok(())
}

/// Wait for a child process to exit and propagate errors.
pub fn wait_for_child(pid: Pid) -> Result<()> {
    debug!(pid = pid.as_raw(), "utils::wait_for_child: waiting");
    waitpid(pid, None).map_err(|e| Error::ProcessManagement {
        operation: "wait for child".to_string(),
        pid: pid.as_raw(),
        details: format!("Failed to wait for child: {e}"),
    })?;
    debug!(pid = pid.as_raw(), "utils::wait_for_child: done");

    Ok(())
}
