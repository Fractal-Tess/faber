use nix::errno::Errno;
use nix::mount::MntFlags;
use nix::{Error as NixError, mount::MsFlags, sched::CloneFlags};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Faber generic error: {source:?}: {details}")]
    Generic {
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
        details: String,
    },

    #[error("Cgroup error: {message}: {details}")]
    Cgroup { message: String, details: String },

    #[error("Unshare failed (flags: {flags:?}): {source}: {details}")]
    Unshare {
        flags: CloneFlags,
        #[source]
        source: NixError,
        details: String,
    },

    #[error(
        "Mount failed: source={src} target={target} fstype={fstype:?} flags={flags:?}: {err}: {details}"
    )]
    Mount {
        src: String,
        target: String,
        fstype: Option<String>,
        flags: MsFlags,
        #[source]
        err: NixError,
        details: String,
    },

    #[error("Umount failed: target={target} flags={flags:?}: {err}: {details}")]
    Umount {
        target: String,
        flags: MntFlags,
        #[source]
        err: NixError,
        details: String,
    },

    #[error("Failed to create directory: {path}: {source}: {details}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
        details: String,
    },

    #[error("Failed to create device node: {path}: {source}: {details}")]
    CreateDeviceNode {
        path: PathBuf,
        #[source]
        source: Errno,
        details: String,
    },

    #[error("Failed to remove directory: {path}: {source}: {details}")]
    RemoveDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
        details: String,
    },

    #[error("Failed to write file: {path} ({bytes} bytes): {source}: {details}")]
    WriteFile {
        path: PathBuf,
        bytes: usize,
        #[source]
        source: std::io::Error,
        details: String,
    },

    #[error("Failed to set hostname '{hostname}': {source}: {details}")]
    SetHostname {
        hostname: String,
        #[source]
        source: NixError,
        details: String,
    },

    #[error("Failed to set group ID to {gid}: {source}: {details}")]
    SetGid {
        gid: u32,
        #[source]
        source: NixError,
        details: String,
    },

    #[error("Failed to set user ID to {uid}: {source}: {details}")]
    SetUid {
        uid: u32,
        #[source]
        source: NixError,
        details: String,
    },

    #[error("Pivot_root failed (new_root={new_root}, old_root={old_root}): {source}: {details}")]
    PivotRoot {
        new_root: PathBuf,
        old_root: PathBuf,
        #[source]
        source: NixError,
        details: String,
    },

    #[error("chdir to '{path}' failed: {source}: {details}")]
    Chdir {
        path: String,
        #[source]
        source: std::io::Error,
        details: String,
    },

    #[error("FFI NulError: {source}: {details}")]
    FFI {
        source: std::ffi::NulError,
        details: String,
    },

    #[error("Nix error: {source}: {details}")]
    Nix { source: NixError, details: String },

    #[error("IO error: {operation} at path '{path}': {details}")]
    Io {
        operation: String,
        path: String,
        details: String,
    },

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
