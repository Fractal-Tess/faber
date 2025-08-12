use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::{CloneFlags, unshare},
    sys::stat::{Mode, SFlag, makedev, mknod},
    unistd::{pivot_root, sethostname},
};

use crate::{
    prelude::*,
    types::{FilesystemConfig, Mount},
};

use std::collections::HashMap;
use std::env::set_current_dir;
use std::fs::{create_dir_all, remove_dir_all, write};
use std::path::{Path, PathBuf};

/// Container environment configuration and management.
///
/// This struct handles the creation, initialization, and cleanup of container
/// environments including namespace isolation, filesystem setup, and device creation.
/// It provides a complete lifecycle for container environments from preparation
/// through cleanup.
#[derive(Debug, Clone)]
pub(crate) struct ContainerEnvironment {
    /// The root directory of the container on the host filesystem.
    /// This path represents where the container's filesystem will be mounted
    /// and serves as the base for all container operations.
    pub(crate) host_container_root: PathBuf,

    /// The hostname to set inside the container.
    /// This provides network identity isolation from the host system.
    pub(crate) hostname: String,

    /// Bind mounts from the host into the container.
    /// These mounts allow the container to access specific host directories
    /// while maintaining isolation boundaries.
    pub(crate) mounts: Vec<Mount>,

    /// The working directory within the container.
    /// This directory serves as the default location for user files and
    /// command execution within the container environment.
    pub(crate) work_dir: String,

    /// Filesystem configuration for tmp and workdir sizes.
    /// Controls the size limits for temporary and working directory filesystems.
    pub(crate) filesystem_config: FilesystemConfig,
}

impl ContainerEnvironment {
    /// Creates a new container environment with the specified configuration.
    ///
    /// This constructor initializes a container environment with the given
    /// root directory, hostname, mount points, working directory, and filesystem
    /// configuration. The environment is not yet prepared for use - call the prepare
    /// methods to set up the actual container filesystem and namespaces.
    ///
    /// # Arguments
    ///
    /// * `container_root` - The root directory path for the container on the host filesystem
    /// * `hostname` - The hostname to set inside the container for network isolation
    /// * `mounts` - List of mount points to bind from the host into the container
    /// * `work_dir` - Working directory within the container environment that will be the default when executing commands
    /// * `filesystem_config` - Configuration for filesystem sizes (tmp and workdir)
    ///
    /// # Returns
    ///
    /// Returns a new `ContainerEnvironment` instance with the specified configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::path::PathBuf;
    /// use faber::types::{Mount, FilesystemConfig};
    ///
    /// let env = ContainerEnvironment::new(
    ///     PathBuf::from("/tmp/container"),
    ///     "my-container".to_string(),
    ///     vec![Mount { source: "/usr/bin".to_string(), target: "/bin".to_string(), flags: vec![], options: vec![], data: None }],
    ///     "workspace".to_string(),
    ///     FilesystemConfig { tmp_size: "64M".to_string(), workdir_size: "128M".to_string() },
    /// );
    /// ```
    pub(crate) fn new(
        container_root: PathBuf,
        hostname: String,
        mounts: Vec<Mount>,
        work_dir: String,
        filesystem_config: FilesystemConfig,
    ) -> Self {
        Self {
            host_container_root: container_root,
            hostname,
            mounts,
            work_dir,
            filesystem_config,
        }
    }

    /// Prepares the container environment before entering the PID namespace.
    ///
    /// This function performs the initial setup of the container environment
    /// and must be called from the parent process before forking into the
    /// container's PID namespace. It handles:
    ///
    /// - Creating the container root directory structure
    /// - Unsharing namespaces for isolation
    /// - Setting up bind mounts from the host
    /// - Performing pivot root to change the filesystem root
    /// - Creating essential device nodes
    /// - Setting up the working directory
    /// - Configuring the container hostname
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any step fails.
    /// The error will contain details about which operation failed and why.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The container root directory cannot be created
    /// - Namespace unsharing fails due to insufficient privileges
    /// - Mount operations fail due to filesystem issues
    /// - Device creation fails due to permission or filesystem constraints
    /// - The working directory cannot be created or accessed
    ///
    /// # Safety
    ///
    /// This function must be called with appropriate privileges (typically root)
    /// and should only be called once per container environment.
    pub(crate) fn prepare_pre_pid_namespace(&self) -> Result<()> {
        // Create the container root
        self.create_container_root_internal()?;

        // Unshare namespaces
        self.unshare_internal()?;

        // Bind mounts
        self.bind_mounts_internal()?;

        // Pivot root
        self.pivot_root_internal()?;

        // Create devices
        self.create_devices_internal()?;

        // Create work directory
        self.create_work_dir_internal(true)?;

        // Set hostname
        self.set_hostname_internal()?;

        Ok(())
    }

    /// Prepares the container environment after entering the PID namespace.
    ///
    /// This function creates the essential filesystems (`/proc`, `/sys`, and `/tmp`)
    /// with appropriate security flags for isolation from the host system.
    /// It must be called from the child process that is already in the
    /// container's PID namespace to avoid leaking the host's proc/sys filesystems.
    ///
    /// The created filesystems provide:
    /// - `/proc`: Process information and system statistics
    /// - `/sys`: Kernel and device information
    /// - `/tmp`: Temporary file storage with size limits
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any filesystem creation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - Directory creation fails due to permission or filesystem issues
    /// - Mount operations fail due to kernel or filesystem constraints
    /// - The process lacks sufficient privileges to create filesystems
    ///
    /// # Safety
    ///
    /// This function must be called from within the container's PID namespace
    /// to ensure proper isolation. Calling it from the parent process will
    /// create security vulnerabilities by exposing host system information.
    pub(crate) fn prepare_post_pid_namespace(&self) -> Result<()> {
        // Create proc
        self.create_proc_internal()?;

        // Create sys
        self.create_sys_internal()?;

        // Create tmp
        self.create_tmp_internal()?;

        Ok(())
    }

    /// Cleans up the container environment by removing the container root directory.
    ///
    /// This function removes all files and directories created during container
    /// initialization, effectively cleaning up the container's filesystem and
    /// freeing up host system resources. It should be called after the container
    /// process has terminated to ensure complete cleanup.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if cleanup fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The container root directory cannot be removed (e.g., still in use)
    /// - Insufficient permissions to remove the directory
    /// - Filesystem errors prevent directory removal
    ///
    /// # Safety
    ///
    /// This function should only be called after ensuring the container process
    /// has completely terminated to avoid removing files that are still in use.
    pub(crate) fn cleanup(&self) -> Result<()> {
        // Remove container root
        remove_dir_all(&self.host_container_root).map_err(|source| Error::RemoveDir {
            path: self.host_container_root.clone(),
            source,
        })?;
        Ok(())
    }

    /// Writes files to the container's working directory.
    ///
    /// Creates the necessary directory structure and writes the provided files
    /// to their respective paths within the container's working directory.
    /// This is typically used to provide source code, configuration files,
    /// or other user content to the container environment.
    ///
    /// # Arguments
    ///
    /// * `files` - HashMap mapping relative file paths (from the working directory) to file contents
    ///             The paths should be relative to the working directory and will be created
    ///             with appropriate parent directories as needed.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any file operations fail.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - Directory creation fails due to permission or filesystem issues
    /// - File writing fails due to disk space or permission constraints
    /// - Invalid file paths are provided
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::collections::HashMap;
    ///
    /// let mut files = HashMap::new();
    /// files.insert("main.rs".to_string(), "fn main() { println!(\"Hello, world!\"); }".to_string());
    /// files.insert("config.toml".to_string(), "[package]\nname = \"my-app\"".to_string());
    ///
    /// env.write_files_to_workdir(&files)?;
    /// ```
    pub(crate) fn write_files_to_workdir(&self, files: &HashMap<String, String>) -> Result<()> {
        // Base path
        let base = PathBuf::from(self.work_dir.trim_start_matches('/'));

        // Create base dir
        create_dir_all(&base).map_err(|source| Error::CreateDir {
            path: base.clone(),
            source,
        })?;

        // Write files
        for (rel_path, contents) in files {
            // Target path
            let target = base.join(rel_path);
            if let Some(parent) = target.parent() {
                // Create parent dir
                create_dir_all(parent).map_err(|source| Error::CreateDir {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }

            // Write file
            write(&target, contents).map_err(|source| Error::WriteFile {
                path: target.clone(),
                bytes: contents.len(),
                source,
            })?;
        }
        Ok(())
    }

    /// Creates the container root directory.
    ///
    /// Creates the base directory structure for the container filesystem on the
    /// host system. This directory will serve as the mount point for the
    /// container's root filesystem and contain all container-related files.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if directory creation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - Insufficient permissions to create the directory
    /// - The parent directory doesn't exist or isn't writable
    /// - Filesystem errors prevent directory creation
    fn create_container_root_internal(&self) -> Result<()> {
        create_dir_all(&self.host_container_root).map_err(|source| Error::CreateDir {
            path: self.host_container_root.clone(),
            source,
        })?;
        Ok(())
    }

    /// Unshares namespaces to isolate the container from the host system.
    ///
    /// Creates new namespaces for complete isolation from the host system:
    /// - **Mount namespace** (`CLONE_NEWNS`): Isolated filesystem view
    /// - **UTS namespace** (`CLONE_NEWUTS`): Isolated hostname and domain name
    /// - **IPC namespace** (`CLONE_NEWIPC`): Isolated inter-process communication
    /// - **PID namespace** (`CLONE_NEWPID`): Isolated process tree
    /// - **Cgroup namespace** (`CLONE_NEWCGROUP`): Isolated resource limits
    /// - **Signal namespace** (`CLONE_SIGHAND`): Isolated signal handling
    /// - **Network namespace** (`CLONE_NEWNET`): Isolated network stack
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if unshare fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The process lacks sufficient privileges (typically requires root)
    /// - The kernel doesn't support the requested namespaces
    /// - System resource limits prevent namespace creation
    ///
    /// # Safety
    ///
    /// This function permanently changes the process's namespace context.
    /// It should only be called once and cannot be undone without process termination.
    fn unshare_internal(&self) -> Result<()> {
        // Unshare flags
        let flags = CloneFlags::CLONE_NEWNS // Mount namespace
            | CloneFlags::CLONE_NEWUTS // UTS namespace
            | CloneFlags::CLONE_NEWIPC // IPC namespace
            | CloneFlags::CLONE_NEWPID // PID namespace
            | CloneFlags::CLONE_NEWCGROUP // Cgroup namespace
            | CloneFlags::CLONE_SIGHAND // Signal namespace
            | CloneFlags::CLONE_NEWNET; // Network namespace

        // Unshare
        unshare(flags).map_err(|source| Error::Unshare { flags, source })?;
        Ok(())
    }

    /// Sets the hostname for the container.
    ///
    /// Changes the system hostname to the value specified in the container configuration.
    /// This provides network identity isolation from the host system and allows the
    /// container to have its own network identity.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if setting hostname fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The process lacks sufficient privileges to change the hostname
    /// - The hostname contains invalid characters
    /// - System calls fail due to kernel constraints
    ///
    /// # Safety
    ///
    /// This function changes the system hostname for the current process and its
    /// descendants. The change is isolated within the UTS namespace.
    fn set_hostname_internal(&self) -> Result<()> {
        // Set hostname
        sethostname(self.hostname.as_str()).map_err(|source| Error::SetHostname {
            hostname: self.hostname.clone(),
            source,
        })?;
        Ok(())
    }

    /// Sets up bind mounts for the container filesystem.
    ///
    /// First makes the root filesystem private to prevent mount propagation,
    /// then creates bind mounts for each mount point specified in the container
    /// configuration. This allows the container to access specific host directories
    /// while maintaining isolation boundaries.
    ///
    /// The function:
    /// 1. Makes the root filesystem private to prevent mount propagation
    /// 2. Iterates through the configured mount points
    /// 3. Creates target directories as needed
    /// 4. Establishes bind mounts with the specified flags
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any mount operation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The root filesystem cannot be made private
    /// - Target directories cannot be created
    /// - Mount operations fail due to filesystem or permission issues
    /// - Invalid mount configurations are provided
    ///
    /// # Safety
    ///
    /// This function modifies the mount namespace and should only be called
    /// after unsharing the mount namespace to ensure proper isolation.
    fn bind_mounts_internal(&self) -> Result<()> {
        // Rebind `/` to make it private
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: "None".to_string(),
            target: "/".to_string(),
            fstype: None,
            flags: MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            err: e,
        })?;

        // Bind mounts
        for m in self.mounts.iter() {
            // Skip if source does not exist
            if !Path::new(&m.source).exists() {
                continue;
            }

            // Target within container
            let target = format!(
                "{}/{target}",
                self.host_container_root.display(),
                target = m.target.strip_prefix("/").unwrap_or(&m.target).to_owned()
            );

            // Mount flags
            let flags = m
                .flags
                .iter()
                .fold(MsFlags::empty(), |acc, flag| acc | *flag);

            // Create target dir
            create_dir_all(&target).map_err(|source| Error::CreateDir {
                path: PathBuf::from(&target),
                source,
            })?;

            // Mount
            mount(
                Some(m.source.as_str()),
                target.as_str(),
                None::<&str>,
                flags,
                m.data.as_deref(),
            )
            .map_err(|e| Error::Mount {
                src: m.source.clone(),
                target: target.clone(),
                fstype: None,
                flags,
                err: e,
            })?;
        }
        Ok(())
    }

    /// Creates the `/proc` filesystem.
    ///
    /// Creates a new `/proc` filesystem with appropriate security flags for
    /// isolation from the host system. This provides process information,
    /// system statistics, and kernel parameters within the container.
    ///
    /// The filesystem is mounted with the following security flags:
    /// - `MS_NODEV`: Prevents access to device files
    /// - `MS_NOSUID`: Ignores set-user-ID and set-group-ID bits
    /// - `MS_NOEXEC`: Prevents execution of files from this filesystem
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any mount operation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The `/proc` directory cannot be created
    /// - The proc filesystem cannot be mounted
    /// - Insufficient privileges to create or mount the filesystem
    ///
    /// # Safety
    ///
    /// This function must be called from within the container's PID namespace
    /// to ensure proper isolation. The created proc filesystem will only
    /// show processes from the container's namespace.
    fn create_proc_internal(&self) -> Result<()> {
        // Proc - mount a new proc filesystem to isolate from host
        let proc_path = "/proc";
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        // Create proc dir
        create_dir_all(proc_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(proc_path),
            source,
        })?;

        // Mount a new proc filesystem (not bind mount from host)
        mount(
            None::<&str>, // No source - create new filesystem
            proc_path,
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: "None".to_string(),
            target: proc_path.to_string(),
            fstype: Some(proc_fstype.to_string()),
            flags: proc_flags,
            err: e,
        })?;

        Ok(())
    }

    /// Creates the `/sys` filesystem.
    ///
    /// Creates a new `/sys` filesystem (sysfs) with appropriate security flags
    /// for isolation from the host system. This provides access to kernel and
    /// device information within the container.
    ///
    /// The filesystem is mounted with the following security flags:
    /// - `MS_NODEV`: Prevents access to device files
    /// - `MS_NOSUID`: Ignores set-user-ID and set-group-ID bits
    /// - `MS_NOEXEC`: Prevents execution of files from this filesystem
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any mount operation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The `/sys` directory cannot be created
    /// - The sysfs cannot be mounted
    /// - Insufficient privileges to create or mount the filesystem
    ///
    /// # Safety
    ///
    /// This function must be called from within the container's PID namespace
    /// to ensure proper isolation. The created sysfs will only show
    /// kernel and device information relevant to the container.
    fn create_sys_internal(&self) -> Result<()> {
        // Sys - mount a new sysfs to isolate from host
        let sys_target = "/sys";
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        // Create sys dir
        create_dir_all(sys_target).map_err(|source| Error::CreateDir {
            path: PathBuf::from(sys_target),
            source,
        })?;
        // Mount a new sysfs (not bind mount from host)
        mount(
            None::<&str>, // No source - create new filesystem
            sys_target,
            Some(sys_fstype),
            sys_flags,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: "None".to_string(),
            target: sys_target.to_string(),
            fstype: Some(sys_fstype.to_string()),
            flags: sys_flags,
            err: e,
        })?;

        Ok(())
    }

    /// Creates and mounts a temporary filesystem.
    ///
    /// Mounts a tmpfs at `/tmp` with a configurable size limit and appropriate permissions
    /// (mode 1777 - readable, writable, and executable by all users). This provides
    /// temporary storage for the container with automatic cleanup when unmounted.
    ///
    /// The tmpfs filesystem:
    /// - Has a configurable size limit to prevent disk space exhaustion
    /// - Uses mode 1777 for full user access (sticky bit prevents deletion of others' files)
    /// - Is automatically cleaned up when the container terminates
    /// - Provides fast access for temporary file operations
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if the mount operation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The `/tmp` directory cannot be created
    /// - The tmpfs cannot be mounted
    /// - Insufficient privileges to create or mount the filesystem
    /// - System memory constraints prevent tmpfs creation
    ///
    /// # Safety
    ///
    /// This function creates a temporary filesystem that will be automatically
    /// cleaned up when unmounted. Files stored in `/tmp` will be lost when
    /// the container terminates.
    fn create_tmp_internal(&self) -> Result<()> {
        let tmp_path = "/tmp";

        // Create tmp dir
        create_dir_all(tmp_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(tmp_path),
            source,
        })?;

        // Mount tmp with configured size
        let mount_options = format!("size={},mode=1777", self.filesystem_config.tmp_size);
        mount(
            Some("tmpfs"),
            tmp_path,
            Some("tmpfs"),
            MsFlags::empty(),
            Some(mount_options.as_str()),
        )
        .map_err(|e| Error::Mount {
            src: "tmpfs".to_string(),
            target: tmp_path.to_string(),
            fstype: Some("tmpfs".to_string()),
            flags: MsFlags::empty(),
            err: e,
        })?;
        Ok(())
    }

    /// Creates the working directory for user files within the container.
    ///
    /// Creates the directory specified in the container configuration where
    /// user files will be placed. This directory serves as the default
    /// location for file operations and command execution within the container.
    /// The working directory is mounted as a tmpfs with configurable size limits
    /// to provide isolated storage for user files.
    ///
    /// # Arguments
    ///
    /// * `change_dir` - Whether to change the current working directory to the work directory
    ///                  If `true`, the process will change to the work directory after creation.
    ///                  If `false`, the directory is created but the current directory remains unchanged.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if directory creation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The working directory cannot be created due to permission or filesystem issues
    /// - The current directory cannot be changed (when `change_dir` is `true`)
    /// - Invalid working directory paths are specified
    /// - The tmpfs cannot be mounted due to insufficient privileges or memory constraints
    ///
    /// # Safety
    ///
    /// When `change_dir` is `true`, this function changes the current working directory
    /// for the process. This change affects all subsequent relative path operations.
    /// The working directory is mounted as a tmpfs, so files will be lost when the
    /// container terminates.
    fn create_work_dir_internal(&self, change_dir: bool) -> Result<()> {
        let work_dir = format!("/{}", self.work_dir.trim_start_matches('/'));

        // Create the work directory
        create_dir_all(&work_dir).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&work_dir),
            source,
        })?;

        // Mount workdir as tmpfs with configured size
        let mount_options = format!("size={},mode=755", self.filesystem_config.workdir_size);
        mount(
            Some("tmpfs"),
            work_dir.as_str(),
            Some("tmpfs"),
            MsFlags::empty(),
            Some(mount_options.as_str()),
        )
        .map_err(|e| Error::Mount {
            src: "tmpfs".to_string(),
            target: work_dir.clone(),
            fstype: Some("tmpfs".to_string()),
            flags: MsFlags::empty(),
            err: e,
        })?;

        // Change directory to the work dir
        if change_dir {
            set_current_dir(&work_dir).map_err(|source| Error::Chdir {
                path: work_dir.clone(),
                source,
            })?;
        }
        Ok(())
    }

    /// Creates essential device nodes for the container.
    ///
    /// Creates the following character devices that are commonly needed
    /// for basic container operations:
    ///
    /// - `/dev/null` - Null device for discarding output (major: 1, minor: 3)
    /// - `/dev/zero` - Zero device for reading zeros (major: 1, minor: 5)
    /// - `/dev/full` - Full device for testing write failures (major: 1, minor: 7)
    /// - `/dev/random` - Random number generator (major: 1, minor: 8)
    /// - `/dev/urandom` - Non-blocking random number generator (major: 1, minor: 9)
    ///
    /// All devices are created with permissions 666 (readable and writable by all users)
    /// and are character devices (`S_IFCHR`).
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any device creation fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The `/dev` directory cannot be created
    /// - Device nodes cannot be created due to permission or filesystem constraints
    /// - Invalid device major/minor numbers are used
    /// - The filesystem doesn't support device node creation
    ///
    /// # Safety
    ///
    /// This function creates device nodes that provide access to system resources.
    /// The devices are created with broad permissions to ensure container processes
    /// can access them, but this also means they should be used carefully.
    fn create_devices_internal(&self) -> Result<()> {
        let flags = SFlag::S_IFCHR;
        let mode = Mode::S_IRUSR
            | Mode::S_IWUSR
            | Mode::S_IRGRP
            | Mode::S_IWGRP
            | Mode::S_IROTH
            | Mode::S_IWOTH;

        let dev_path = "/dev";
        create_dir_all(dev_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(dev_path),
            source,
        })?;

        // Create null device
        let device_path = format!("{dev_path}/null");
        let device_id = makedev(1, 3);
        mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
            Error::FileSystem {
                operation: "create device node".to_string(),
                path: device_path.clone(),
                details: format!("Failed to create null device: {source}"),
            }
        })?;

        // Create zero device
        let device_path = format!("{dev_path}/zero");
        let device_id = makedev(1, 5);
        mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
            Error::FileSystem {
                operation: "create device node".to_string(),
                path: device_path.clone(),
                details: format!("Failed to create zero device: {source}"),
            }
        })?;

        // Create full device
        let device_path = format!("{dev_path}/full");
        let device_id = makedev(1, 7);
        mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
            Error::FileSystem {
                operation: "create device node".to_string(),
                path: device_path.clone(),
                details: format!("Failed to create full device: {source}"),
            }
        })?;

        // Create random device
        let device_path = format!("{dev_path}/random");
        let device_id = makedev(1, 8);
        mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
            Error::FileSystem {
                operation: "create device node".to_string(),
                path: device_path.clone(),
                details: format!("Failed to create random device: {source}"),
            }
        })?;

        // Create urandom device
        let device_path = format!("{dev_path}/urandom");
        let device_id = makedev(1, 9);
        mknod(device_path.as_str(), flags, mode, device_id).map_err(|source| {
            Error::FileSystem {
                operation: "create device node".to_string(),
                path: device_path.clone(),
                details: format!("Failed to create urandom device: {source}"),
            }
        })?;

        Ok(())
    }

    /// Performs a pivot root operation to change the filesystem root.
    ///
    /// This function changes the root filesystem from the host root to the
    /// container directory, effectively isolating the container's view of
    /// the filesystem. The operation follows these steps:
    ///
    /// 1. Creates an `oldroot` directory within the container for the pivot operation
    /// 2. Bind mounts the new root (container directory) to itself to maintain references
    /// 3. Changes the root filesystem to the container directory using `pivot_root`
    /// 4. Changes the current working directory to the new root (`/`)
    /// 5. Unmounts the old root to complete the isolation
    /// 6. Removes the `oldroot` directory to clean up
    ///
    /// After this operation, the container process will see the container directory
    /// as its root filesystem (`/`), providing complete filesystem isolation.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an `Error` if any step fails.
    ///
    /// # Errors
    ///
    /// This function may fail if:
    /// - The `oldroot` directory cannot be created
    /// - Bind mounting the new root fails
    /// - The `pivot_root` system call fails
    /// - Changing the current directory fails
    /// - Unmounting the old root fails
    /// - Removing the `oldroot` directory fails
    ///
    /// # Safety
    ///
    /// This function permanently changes the root filesystem for the current process
    /// and cannot be undone without process termination. It should only be called
    /// after proper namespace isolation is established.
    ///
    /// # Notes
    ///
    /// The pivot root operation is a critical security feature that ensures the
    /// container cannot access files outside its designated filesystem. After this
    /// operation, the container's view of the filesystem is completely isolated
    /// from the host system.
    fn pivot_root_internal(&self) -> Result<()> {
        // New root path (which is essentially the container root)
        let new_root = self.host_container_root.clone();

        // Old root path (which is /oldroot -> host root `/`)
        let old_root = format!("{}/oldroot", self.host_container_root.display());

        // Create old root
        create_dir_all(&old_root).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&old_root),
            source,
        })?;

        // Bind mount new root to itself
        let new_root_str = new_root.to_str().ok_or_else(|| Error::Configuration {
            component: "container root path".to_string(),
            details: "Container root path contains invalid UTF-8 characters".to_string(),
        })?;

        mount(
            Some(new_root_str),
            new_root_str,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: new_root_str.to_string(),
            target: new_root_str.to_string(),
            fstype: None,
            flags: MsFlags::MS_BIND | MsFlags::MS_REC,
            err: e,
        })?;

        // Pivot root
        pivot_root(new_root_str, old_root.as_str()).map_err(|source| Error::PivotRoot {
            new_root: new_root.clone(),
            old_root: PathBuf::from(&old_root),
            source,
        })?;

        // Set current directory to the root of the container which is now `/`
        set_current_dir("/").map_err(|source| Error::Chdir {
            path: "/".to_string(),
            source,
        })?;

        // Umount old root
        umount2("/oldroot", MntFlags::MNT_DETACH).map_err(|e| Error::Umount {
            target: "/oldroot".to_string(),
            flags: MntFlags::MNT_DETACH | MntFlags::MNT_FORCE,
            err: e,
        })?;

        // Remove old root
        remove_dir_all("/oldroot").map_err(|source| Error::RemoveDir {
            path: PathBuf::from("/oldroot"),
            source,
        })?;

        Ok(())
    }
}
