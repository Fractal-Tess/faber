use nix::mount::MntFlags;
use nix::{Error as NixError, mount::MsFlags, sched::CloneFlags};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Faber generic error: {0}")]
    GenericError(String),

    #[error("Unshare failed (flags: {flags:?}): {source}")]
    Unshare {
        flags: CloneFlags,
        #[source]
        source: NixError,
    },

    #[error("Mount failed: source={src} target={target} fstype={fstype:?} flags={flags:?}: {err}")]
    Mount {
        src: String,
        target: String,
        fstype: Option<String>,
        flags: MsFlags,
        #[source]
        err: NixError,
    },

    #[error("Umount failed: target={target} flags={flags:?}: {err}")]
    Umount {
        target: String,
        flags: MntFlags,
        #[source]
        err: NixError,
    },

    #[error("Failed to create directory: {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to remove directory: {path}: {source}")]
    RemoveDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file: {path} ({bytes} bytes): {source}")]
    WriteFile {
        path: PathBuf,
        bytes: usize,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to set hostname '{hostname}': {source}")]
    SetHostname {
        hostname: String,
        #[source]
        source: NixError,
    },

    #[error("Pivot_root failed (new_root={new_root}, old_root={old_root}): {source}")]
    PivotRoot {
        new_root: PathBuf,
        old_root: PathBuf,
        #[source]
        source: NixError,
    },

    #[error("chdir to '{path}' failed: {source}")]
    Chdir {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("FFI NulError: {0}")]
    FFINullError(#[from] std::ffi::NulError),

    #[error("Nix error: {0}")]
    NixError(#[from] NixError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    // Cgroup-specific errors
    #[error("Failed to create cgroup dir: {path}: {source}")]
    CgroupCreate {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write cgroup file: {path} (value={value}): {source}")]
    CgroupWrite {
        path: PathBuf,
        value: String,
        #[source]
        source: std::io::Error,
    },
}
