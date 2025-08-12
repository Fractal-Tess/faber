use crate::prelude::*;
use nix::{sys::wait::waitpid, unistd::Pid};
use std::{
    io::{PipeReader, PipeWriter, pipe},
    os::fd::RawFd,
};

pub fn mk_pipe() -> Result<(PipeReader, PipeWriter)> {
    pipe().map_err(|e| Error::ProcessManagement {
        operation: "create pipe".to_string(),
        pid: -1,
        details: format!("Failed to create pipe: {e}"),
    })
}

pub fn close_fd(fd: RawFd) -> Result<()> {
    let _ = fd;

    Ok(())
}

pub fn wait_for_child(pid: Pid) -> Result<()> {
    let _ = waitpid(pid, None).map_err(|e| Error::ProcessManagement {
        operation: "wait for child".to_string(),
        pid: pid.as_raw(),
        details: format!("Failed to wait for child: {e}"),
    });

    Ok(())
}
