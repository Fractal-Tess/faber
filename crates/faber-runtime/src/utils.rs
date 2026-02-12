use std::{
    io::{pipe, PipeReader, PipeWriter},
    os::fd::RawFd,
};

use nix::unistd::close;

use crate::prelude::*;

pub fn generate_random_string(size: u8) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..size)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub fn mk_pipe() -> Result<(PipeReader, PipeWriter)> {
    pipe().map_err(|e| FaberError::MkPipe {
        e,
        details: "Failed to create pipe".to_string(),
    })
}

pub fn close_fd(fd: RawFd) -> Result<()> {
    close(fd).map_err(|e| FaberError::CloseFd { e })?;

    Ok(())
}
