use nix::sched::CloneFlags;
use std::path::PathBuf;

/// Comprehensive error type for container operations.
///
/// This enum represents all possible errors that can occur during container lifecycle
/// operations including filesystem setup, mounting, process execution, and cleanup.
/// Each variant includes contextual information to aid in debugging and error reporting.
///
/// The errors are organized into logical groups:
/// - **Filesystem Operations**: Directory/file creation, permissions, writing
/// - **Mount Operations**: Binding, mounting, and unmounting filesystems
/// - **Process Operations**: Forking, execution, waiting, and IPC
/// - **System Operations**: Low-level system calls and resource management
#[derive(thiserror::Error, Debug)]
pub enum ContainerError {
    // === Filesystem Operations ===
    /// Failed to create a directory in the container filesystem.
    ///
    /// This error occurs when creating directories for the container root,
    /// mount points, or parent directories for files. Common causes include
    /// permission denied, disk full, or invalid path.
    #[error("Create directory {path}: {source}")]
    CreateDir {
        /// The path that failed to be created
        path: PathBuf,
        #[source]
        /// The underlying I/O error from the filesystem
        source: std::io::Error,
    },

    /// Failed to set filesystem permissions on a path.
    ///
    /// This error occurs when attempting to set security permissions on
    /// directories or files, typically during container root setup.
    /// The octal mode shows the intended permission bits.
    #[error("Set permissions 0{octal_mode:o} on {path}: {source}")]
    SetPermissions {
        /// The path where permission setting failed
        path: PathBuf,
        /// The intended permission bits in octal format
        octal_mode: u32,
        #[source]
        /// The underlying I/O error from the filesystem
        source: std::io::Error,
    },

    /// Failed to create a file in the container filesystem.
    ///
    /// This error occurs when creating placeholder files for bind mounts
    /// or when writing user-provided files to the container.
    #[error("Create file {path}: {source}")]
    CreateFile {
        /// The file path that failed to be created
        path: PathBuf,
        #[source]
        /// The underlying I/O error from the filesystem
        source: std::io::Error,
    },

    /// Failed to write content to a file.
    ///
    /// This error occurs when writing user-provided file contents
    /// to the container's work directory.
    #[error("Write file {path}: {source}")]
    WriteFile {
        /// The file path where writing failed
        path: PathBuf,
        #[source]
        /// The underlying I/O error from the write operation
        source: std::io::Error,
    },

    /// Failed to stat a path to retrieve metadata.
    ///
    /// This error occurs when querying file metadata (e.g., to discover device
    /// major/minor numbers before creating a device node).
    #[error("Stat {path}: {source}")]
    StatPath {
        /// The path that failed to be stat'ed
        path: PathBuf,
        #[source]
        /// The underlying stat system call error
        source: nix::Error,
    },

    /// Failed to create a device node with mknod.
    ///
    /// This error occurs when creating character or block device nodes inside
    /// the container filesystem.
    #[error("Create device {path}: {source}")]
    CreateDevice {
        /// The device node path that failed to be created
        path: PathBuf,
        #[source]
        /// The underlying mknod system call error
        source: nix::Error,
    },

    /// Failed to remove an existing path.
    ///
    /// This error occurs when attempting to remove a pre-existing file at a
    /// device mount target before re-creating it as a device node.
    #[error("Remove path {path}: {source}")]
    RemovePath {
        /// The path that failed to be removed
        path: PathBuf,
        #[source]
        /// The underlying I/O error from the remove operation
        source: std::io::Error,
    },

    // === Mount Operations - Folders ===
    /// Failed to mount a folder using bind mount.
    ///
    /// This error occurs during the initial bind mount operation when
    /// attaching a host directory to the container filesystem.
    #[error("Mount folder '{name}' {src} -> {tgt}: {source}")]
    MountFolder {
        /// Human-readable name of the mount from configuration
        name: String,
        /// Source path on the host filesystem
        src: String,
        /// Target path within the container
        tgt: PathBuf,
        #[source]
        /// The underlying mount system call error
        source: nix::Error,
    },

    /// Failed to remount a folder with security restrictions.
    ///
    /// This error occurs when applying security flags (read-only, nosuid, nodev)
    /// to an already mounted folder.
    #[error("Remount folder '{name}' at {tgt} read-only: {source}")]
    RemountFolder {
        /// Human-readable name of the mount from configuration
        name: String,
        /// Target path within the container being remounted
        tgt: PathBuf,
        #[source]
        /// The underlying remount system call error
        source: nix::Error,
    },

    /// Failed to mount a tmpfs filesystem.
    ///
    /// This error occurs when creating temporary filesystems for
    /// work directories or other ephemeral storage needs.
    #[error("Mount tmpfs '{name}' at {tgt} with opts '{options}': {source}")]
    MountTmpfs {
        /// Human-readable name of the tmpfs mount
        name: String,
        /// Target path within the container
        tgt: PathBuf,
        /// Mount options passed to the tmpfs (size limits, etc.)
        options: String,
        #[source]
        /// The underlying mount system call error
        source: nix::Error,
    },

    // === Mount Operations - Devices ===
    /// Failed to mount a device file using bind mount.
    ///
    /// This error occurs when binding device files (like /dev/null)
    /// into the container for controlled device access.
    #[error("Mount device '{name}' {src} -> {tgt}: {source}")]
    MountDevice {
        /// Human-readable name of the device mount
        name: String,
        /// Source device path on the host
        src: String,
        /// Target device path within the container
        tgt: PathBuf,
        #[source]
        /// The underlying mount system call error
        source: nix::Error,
    },

    /// Failed to remount a device with read-only restrictions.
    ///
    /// This error occurs when applying security restrictions to
    /// device mounts to prevent unauthorized device access.
    #[error("Remount device '{name}' at {tgt} read-only: {source}")]
    RemountDevice {
        /// Human-readable name of the device mount
        name: String,
        /// Target device path being remounted
        tgt: PathBuf,
        #[source]
        /// The underlying remount system call error
        source: nix::Error,
    },

    // === Mount Operations - Files ===
    /// Failed to mount a single file using bind mount.
    ///
    /// This error occurs when binding individual files (like configuration files)
    /// into the container filesystem.
    #[error("Mount file '{name}' {src} -> {tgt}: {source}")]
    MountFile {
        /// Human-readable name of the file mount
        name: String,
        /// Source file path on the host
        src: String,
        /// Target file path within the container
        tgt: PathBuf,
        #[source]
        /// The underlying mount system call error
        source: nix::Error,
    },

    /// Failed to remount a file with security restrictions.
    ///
    /// This error occurs when applying security flags to file mounts
    /// to enforce read-only access or other restrictions.
    #[error("Remount file '{name}' at {tgt} read-only: {source}")]
    RemountFile {
        /// Human-readable name of the file mount
        name: String,
        /// Target file path being remounted
        tgt: PathBuf,
        #[source]
        /// The underlying remount system call error
        source: nix::Error,
    },

    // === Mount Operations - System ===
    /// Failed to set private mount propagation.
    ///
    /// This error occurs when isolating mount namespaces to prevent
    /// mount events from propagating outside the container.
    #[error("Set private mount propagation on {tgt}: {source}")]
    SetPrivate {
        /// The mount point where propagation setting failed
        tgt: PathBuf,
        #[source]
        /// The underlying mount system call error
        source: nix::Error,
    },

    /// Failed to mount the /proc filesystem.
    ///
    /// This error occurs when mounting the process information filesystem,
    /// which is essential for many system utilities and process management.
    #[error("Mount /proc at {tgt}: {source}")]
    MountProc {
        /// The target path where /proc mount failed
        tgt: PathBuf,
        #[source]
        /// The underlying mount system call error
        source: nix::Error,
    },

    /// Failed to mount the /sys filesystem.
    ///
    /// This error occurs when mounting the system information filesystem,
    /// which provides access to kernel and hardware information.
    #[error("Mount /sys at {tgt}: {source}")]
    MountSys {
        /// The target path where /sys mount failed
        tgt: PathBuf,
        #[source]
        /// The underlying mount system call error
        source: nix::Error,
    },

    // === Process Operations - High Level ===
    /// Failed to spawn a process using tokio's spawn_blocking.
    ///
    /// This error occurs when the async runtime fails to create a blocking
    /// task for container execution, typically due to resource exhaustion.
    #[error("Spawn '{cmd}': {source}")]
    Spawn {
        /// The command that was being spawned
        cmd: String,
        #[source]
        /// The underlying I/O error from the spawn operation
        source: std::io::Error,
    },

    /// Failed to wait for a process to complete.
    ///
    /// This error occurs when waiting for a child process fails,
    /// which may indicate the process was killed or other system issues.
    #[error("Wait for '{cmd}': {source}")]
    Wait {
        /// The command that was being waited for
        cmd: String,
        #[source]
        /// The underlying I/O error from the wait operation
        source: std::io::Error,
    },

    // === Process Operations - Low Level ===
    /// Failed to fork a new process.
    ///
    /// This error occurs when the fork system call fails, typically due to
    /// resource limits, memory exhaustion, or system constraints.
    #[error("Fork failed: {0}")]
    Fork(#[source] nix::Error),

    /// Failed to create Linux namespaces.
    ///
    /// This error occurs when the unshare system call fails to create
    /// isolation namespaces. Common causes include insufficient privileges
    /// or kernel configuration issues.
    #[error("Unshare failed ({flags:?}): {source}")]
    Unshare {
        /// The namespace flags that failed to be created
        flags: CloneFlags,
        #[source]
        /// The underlying system call error
        source: nix::Error,
    },

    /// Failed to create a pipe for inter-process communication.
    ///
    /// This error occurs when creating pipes for stdout/stderr capture
    /// between parent and child processes.
    #[error("Pipe failed: {0}")]
    Pipe(#[source] nix::Error),

    /// Failed to duplicate a file descriptor.
    ///
    /// This error occurs when redirecting stdout/stderr to pipes using dup2,
    /// which is essential for capturing process output.
    #[error("dup2 {fd} -> {target}: {source}")]
    Dup2 {
        /// The source file descriptor being duplicated
        fd: i32,
        /// The target file descriptor number
        target: i32,
        #[source]
        /// The underlying dup2 system call error
        source: nix::Error,
    },

    /// Failed to execute a command.
    ///
    /// This error occurs when execvpe fails to replace the process image
    /// with the target command. Common causes include command not found,
    /// permission denied, or invalid executable format.
    #[error("exec '{cmd}': {source}")]
    Exec {
        /// The command that failed to execute
        cmd: String,
        #[source]
        /// The underlying exec system call error
        source: nix::Error,
    },

    /// Failed to wait for a child process.
    ///
    /// This error occurs when the waitpid system call fails,
    /// which may indicate the child process doesn't exist or other issues.
    #[error("waitpid {pid}: {source}")]
    WaitPid {
        /// The process ID that was being waited for
        pid: i32,
        #[source]
        /// The underlying waitpid system call error
        source: nix::Error,
    },

    /// Failed to convert a string to a null-terminated C string.
    ///
    /// This error occurs when strings contain null bytes, which are not
    /// allowed in C strings used for system calls like exec and environment variables.
    #[error("CString conversion for '{value}': {source}")]
    CString {
        /// The string value that contained null bytes
        value: String,
        #[source]
        /// The underlying null byte error
        source: std::ffi::NulError,
    },

    // === Cleanup Operations ===
    /// Failed to unmount a filesystem.
    ///
    /// This error occurs during container cleanup when attempting to
    /// unmount bound filesystems, devices, or tmpfs mounts.
    #[error("Unmount {tgt}: {source}")]
    Unmount {
        /// The mount point that failed to be unmounted
        tgt: PathBuf,
        #[source]
        /// The underlying unmount system call error
        source: nix::Error,
    },

    /// Failed to remove a directory during cleanup.
    ///
    /// This error occurs when cleaning up the container root directory
    /// after unmounting all filesystems.
    #[error("Remove directory {path}: {source}")]
    RemoveDir {
        /// The directory path that failed to be removed
        path: PathBuf,
        #[source]
        /// The underlying I/O error from directory removal
        source: std::io::Error,
    },
}
