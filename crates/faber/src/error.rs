use nix::mount::MntFlags;
use nix::{Error as NixError, mount::MsFlags, sched::CloneFlags};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Faber generic error: {0}")]
    Generic(String),

    #[error("Cgroup error: {0}")]
    Cgroup(String),

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
    FFI(#[from] std::ffi::NulError),

    #[error("Nix error: {0}")]
    Nix(#[from] NixError),

    #[error("IO error: {operation} at path '{path}': {details}")]
    Io {
        operation: String,
        path: String,
        details: String,
    },

    // New specific error types for better error handling
    #[error("Process management error: {operation} (pid: {pid}): {details}")]
    ProcessManagement {
        operation: String,
        pid: i32,
        details: String,
    },

    #[error("Thread management error: {operation}: {details}")]
    ThreadManagement { operation: String, details: String },

    #[error("Serialization error: {operation} for data type '{data_type}': {details}")]
    Serialization {
        operation: String,
        data_type: String,
        details: String,
    },

    #[error("Deserialization error: {operation} for data type '{data_type}': {details}")]
    Deserialization {
        operation: String,
        data_type: String,
        details: String,
    },

    #[error("File descriptor error: {operation} (fd: {fd}): {details}")]
    FileDescriptor {
        operation: String,
        fd: i32,
        details: String,
    },

    #[error("Task execution error: command='{command}' args={args:?}: {details}")]
    TaskExecution {
        command: String,
        args: Option<Vec<String>>,
        details: String,
    },

    #[error("Container environment error: {operation}: {details}")]
    ContainerEnvironment { operation: String, details: String },

    #[error("Resource limit error: {resource_type} limit exceeded: {details}")]
    ResourceLimit {
        resource_type: String,
        details: String,
    },

    #[error("Timeout error: operation '{operation}' exceeded {timeout_secs}s: {details}")]
    Timeout {
        operation: String,
        timeout_secs: u64,
        details: String,
    },

    #[error("Validation error: {field} - {details}")]
    Validation { field: String, details: String },

    #[error("Configuration error: {component} - {details}")]
    Configuration { component: String, details: String },

    #[error("File system error: {operation} at path '{path}': {details}")]
    FileSystem {
        operation: String,
        path: String,
        details: String,
    },

    #[error(
        "Mount point error: {operation} for mount '{mount_name}' (source: '{mount_source}', target: '{target}'): {details}"
    )]
    MountPoint {
        operation: String,
        mount_name: String,
        mount_source: String,
        target: String,
        details: String,
    },

    #[error("Directory access error: {operation} for directory '{path}': {details}")]
    DirectoryAccess {
        operation: String,
        path: String,
        details: String,
    },
}
