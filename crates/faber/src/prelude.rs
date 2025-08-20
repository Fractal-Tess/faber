use std::io::{PipeReader, PipeWriter};
use std::os::fd::IntoRawFd;

pub use crate::error::Error;

pub type Result<T> = std::result::Result<T, Error>;

pub trait Closable {
    fn close(self) -> Result<()>;
}

impl Closable for PipeReader {
    fn close(self) -> Result<()> {
        let fd = self.into_raw_fd();
        nix::unistd::close(fd).map_err(|e| Error::Pipe {
            details: format!("Failed to close fd of pipe reader: {fd}: {e}"),
            error: e,
        })?;

        Ok(())
    }
}

impl Closable for PipeWriter {
    fn close(self) -> Result<()> {
        let fd = self.into_raw_fd();
        nix::unistd::close(fd).map_err(|e| Error::Pipe {
            details: format!("Failed to close fd of pipe writer: {fd}: {e}"),
            error: e,
        })?;

        Ok(())
    }
}
