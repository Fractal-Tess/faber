//! Container runtime management and lifecycle operations.
//!
//! This module provides the high-level `ContainerRuntime` interface for managing
//! container instances throughout their lifecycle. It orchestrates filesystem setup,
//! mount operations, file management, and isolated command execution.
//!
//! # Container Lifecycle
//!
//! A typical container lifecycle follows this pattern:
//!
//! 1. **Creation**: `ContainerRuntime::new()` - Initialize runtime with configuration
//! 2. **Preparation**: `prepare()` - Set up filesystem and mounts
//! 3. **File Writing**: `write_files()` - Add user files to work directory
//! 4. **Execution**: `run_command()` - Execute commands in isolation
//! 5. **Cleanup**: `cleanup()` - Unmount filesystems and remove directories
//!
//! # Security Model
//!
//! The container runtime implements multiple layers of security:
//! - **Filesystem isolation**: chroot to container root directory
//! - **Namespace isolation**: Separate PID, mount, network, IPC, and UTS namespaces
//! - **Mount restrictions**: nosuid, nodev, and optional read-only mounts
//! - **Permission control**: Restricted directory permissions (0o700)
//!
//! # Example Usage
//!
//! ```rust
//! use std::collections::HashMap;
//!
//! // Create and prepare container
//! let runtime = ContainerRuntime::new(fs_config, "request-123");
//! runtime.prepare()?;
//!
//! // Add files and execute command
//! let files = HashMap::from([("main.py".to_string(), "print('Hello')".to_string())]);
//! runtime.write_files(&files)?;
//! let (stdout, stderr, exit_code) = runtime.run_command("python3", &["main.py".to_string()], &HashMap::new()).await?;
//!
//! // Clean up
//! runtime.cleanup()?;
//! ```

use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tracing::{debug, info};

use super::errors::ContainerError;
use super::execution;
use super::mounts;
use crate::config::ContainerFilesystemConfig;

/// High-level container runtime for managing isolated execution environments.
///
/// `ContainerRuntime` provides a complete interface for creating, managing, and
/// cleaning up containerized execution environments. Each instance represents
/// a single container with its own isolated filesystem, namespaces, and execution context.
///
/// The runtime manages:
/// - **Filesystem isolation**: Creates and manages the container's root filesystem
/// - **Mount management**: Sets up bind mounts, tmpfs, devices, and essential filesystems
/// - **File operations**: Writes user-provided files into the container
/// - **Process execution**: Runs commands in complete isolation using Linux namespaces
/// - **Cleanup**: Safely tears down all container resources
///
/// # Thread Safety
///
/// `ContainerRuntime` is `Clone` and can be used across async contexts, but
/// operations on the same container should be coordinated to avoid conflicts.
///
/// # Resource Management
///
/// The runtime automatically manages all container resources. Always call
/// `cleanup()` when done to ensure proper resource cleanup, even if operations fail.
#[derive(Debug, Clone)]
pub struct ContainerRuntime {
    /// Unique identifier for this container instance
    request_id: String,
    /// Absolute path to the container's root directory on the host
    root: PathBuf,
    /// Filesystem configuration specifying mounts and directories
    fs_cfg: ContainerFilesystemConfig,
}

impl ContainerRuntime {
    /// Creates a new container runtime instance without touching the filesystem.
    ///
    /// This constructor initializes the runtime configuration but does not create
    /// any directories or perform any filesystem operations. The actual container
    /// setup happens during the `prepare()` call.
    ///
    /// The container root directory is determined by joining the base directory
    /// from the configuration with the provided request ID, ensuring each
    /// container has a unique filesystem location.
    ///
    /// # Arguments
    /// * `fs_cfg` - Filesystem configuration specifying base directory, mounts, and permissions
    /// * `request_id` - Unique identifier for this container (often a request/job ID)
    ///
    /// # Returns
    /// A new `ContainerRuntime` instance ready for preparation
    ///
    /// # Examples
    /// ```rust
    /// let runtime = ContainerRuntime::new(filesystem_config, "job-12345");
    /// // Container is configured but not yet created
    /// ```
    ///
    /// # Performance
    /// This operation is lightweight and only performs path calculations and
    /// struct initialization. No I/O operations are performed.
    pub fn new(fs_cfg: ContainerFilesystemConfig, request_id: impl Into<String>) -> Self {
        let request_id = request_id.into();
        let root = Path::new(&fs_cfg.base_dir).join(&request_id);
        debug!("Container root: {}", root.display());
        Self {
            request_id,
            root,
            fs_cfg,
        }
    }

    /// Prepares the container by creating the root directory and setting up all mounts.
    ///
    /// This method performs the complete filesystem setup required for container
    /// isolation. It creates the container root directory with restrictive permissions
    /// and sets up all configured mounts including folders, tmpfs, devices, files,
    /// and essential system filesystems.
    ///
    /// The preparation process:
    /// 1. Creates the container root directory with 0o700 permissions
    /// 2. Creates base directories for tmpfs mounts (work, tmp)
    /// 3. Sets up all configured mounts in the correct order
    /// 4. Applies security restrictions to all mounts
    ///
    /// # Returns
    /// * `Ok(())` - Container is fully prepared and ready for use
    /// * `Err(ContainerError)` - Preparation failed, container is not usable
    ///
    /// # Errors
    /// - `ContainerError::CreateDir` - Failed to create directories
    /// - `ContainerError::SetPermissions` - Failed to set directory permissions  
    /// - Various mount errors - Failed to set up filesystem mounts
    ///
    /// # Examples
    /// ```rust
    /// let runtime = ContainerRuntime::new(config, "job-123");
    /// runtime.prepare().expect("Container preparation failed");
    /// // Container is now ready for file operations and execution
    /// ```
    ///
    /// # Side Effects
    /// - Creates directories on the host filesystem
    /// - Performs mount operations that require elevated privileges
    /// - May create device files and bind mounts
    pub fn prepare(&self) -> Result<(), ContainerError> {
        info!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Preparing container"
        );
        self.create_container_root()?;
        self.setup_mounts()?;
        debug!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Container prepared"
        );
        Ok(())
    }

    /// Cleans up the container by unmounting all filesystems and removing directories.
    ///
    /// This method performs a complete teardown of the container environment.
    /// It safely unmounts all filesystems in reverse order and removes the
    /// container root directory, freeing all associated resources.
    ///
    /// The cleanup process:
    /// 1. Unmounts all filesystems (files, devices, tmpfs, folders)
    /// 2. Removes the entire container root directory tree
    /// 3. Frees any associated system resources
    ///
    /// # Returns
    /// * `Ok(())` - Container was successfully cleaned up
    /// * `Err(ContainerError)` - Cleanup failed, some resources may remain
    ///
    /// # Errors
    /// - `ContainerError::Unmount` - Failed to unmount filesystems
    /// - `ContainerError::RemoveDir` - Failed to remove container directory
    ///
    /// # Error Handling
    /// The cleanup process continues even if some operations fail, attempting
    /// to clean up as many resources as possible. Warnings are logged for
    /// individual failures.
    ///
    /// # Examples
    /// ```rust
    /// // Always clean up, even after errors
    /// let result = runtime.run_command("echo", &["hello".to_string()], &HashMap::new()).await;
    /// runtime.cleanup().expect("Cleanup failed");
    /// ```
    ///
    /// # Resource Safety
    /// This method should always be called when done with a container,
    /// regardless of whether previous operations succeeded or failed.
    pub fn cleanup(&self) -> Result<(), ContainerError> {
        info!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Cleaning up container"
        );
        self.umount_all()?;
        fs::remove_dir_all(&self.root).map_err(|e| ContainerError::RemoveDir {
            path: self.root.clone(),
            source: e,
        })?;
        debug!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Container cleaned up"
        );
        Ok(())
    }

    /// Returns the absolute path to the container's root directory.
    ///
    /// This method provides access to the container's root directory path
    /// on the host filesystem. This path represents the top-level directory
    /// that will become "/" inside the container after chroot.
    ///
    /// # Returns
    /// A reference to the container root path
    ///
    /// # Examples
    /// ```rust
    /// let runtime = ContainerRuntime::new(config, "job-123");
    /// println!("Container root: {}", runtime.root().display());
    /// // Output: Container root: /tmp/containers/job-123
    /// ```
    ///
    /// # Use Cases
    /// - Debugging and logging container locations
    /// - External tools that need to inspect container filesystems
    /// - Backup or archival operations
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes user-provided files into the container's work directory.
    ///
    /// This method takes a collection of files (as path-content pairs) and
    /// writes them into the container's work directory. Parent directories
    /// are created automatically as needed. Files are written to the host-side
    /// view of the container filesystem before execution begins.
    ///
    /// The files will be available inside the container at their relative
    /// paths within the work directory. For example, a file "src/main.py"
    /// will be accessible as "/work/src/main.py" inside the container
    /// (assuming work directory is mounted at "/work").
    ///
    /// # Arguments
    /// * `files` - HashMap mapping relative file paths to their string contents
    ///
    /// # Returns
    /// * `Ok(())` - All files were written successfully
    /// * `Err(ContainerError)` - File writing failed
    ///
    /// # Errors
    /// - `ContainerError::CreateDir` - Failed to create parent directories
    /// - `ContainerError::WriteFile` - Failed to write file contents
    ///
    /// # Examples
    /// ```rust
    /// use std::collections::HashMap;
    ///
    /// let mut files = HashMap::new();
    /// files.insert("main.py".to_string(), "print('Hello, World!')".to_string());
    /// files.insert("data/input.txt".to_string(), "sample data".to_string());
    ///
    /// runtime.write_files(&files)?;
    /// // Files are now available in the container's work directory
    /// ```
    ///
    /// # Performance Considerations
    /// - Creates parent directories only as needed
    /// - Writes files sequentially (not parallelized)
    /// - Logs detailed information about each file operation
    /// - Returns early if the files collection is empty
    pub fn write_files(&self, files: &HashMap<String, String>) -> Result<(), ContainerError> {
        info!(
            request_id = %self.request_id,
            root = %self.root.display(),
            file_count = files.len(),
            "Writing files into container work directory"
        );
        if files.is_empty() {
            return Ok(());
        }
        let host_work_dir = self.root.join(&self.fs_cfg.work_dir.target);
        for (relative_path, content) in files {
            let dest = host_work_dir.join(relative_path);
            debug!(
                request_id = %self.request_id,
                path = %dest.display(),
                bytes = content.len(),
                "Writing file"
            );
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| ContainerError::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            fs::write(&dest, content).map_err(|e| ContainerError::WriteFile {
                path: dest.clone(),
                source: e,
            })?;
        }
        debug!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Finished writing files"
        );
        Ok(())
    }

    /// Executes a command within the isolated container environment.
    ///
    /// This method runs a command inside the fully isolated container using
    /// Linux namespaces, chroot, and other security mechanisms. The execution
    /// happens in a separate async task to avoid blocking the current thread.
    ///
    /// The isolation includes:
    /// - **Filesystem isolation**: Command runs in chroot environment
    /// - **Process isolation**: Separate PID namespace  
    /// - **Network isolation**: Isolated network namespace
    /// - **IPC isolation**: Separate inter-process communication
    /// - **Hostname isolation**: Separate UTS namespace
    /// - **Mount isolation**: Private mount namespace
    ///
    /// # Arguments
    /// * `cmd` - Command name to execute (searched in PATH)
    /// * `args` - Command-line arguments to pass to the command
    /// * `env` - Environment variables to set for the command
    ///
    /// # Returns
    /// * `Ok((stdout, stderr, exit_code))` - Command completed successfully
    ///   - `stdout` - Standard output captured as UTF-8 string
    ///   - `stderr` - Standard error captured as UTF-8 string  
    ///   - `exit_code` - Process exit code (0 for success)
    /// * `Err(ContainerError)` - Command execution failed
    ///
    /// # Errors
    /// - `ContainerError::Spawn` - Failed to spawn the blocking task
    /// - Various execution errors - From the underlying isolation mechanism
    ///
    /// # Examples
    /// ```rust
    /// use std::collections::HashMap;
    ///
    /// // Run a simple command
    /// let (stdout, stderr, exit_code) = runtime.run_command(
    ///     "echo",
    ///     &["Hello, World!".to_string()],
    ///     &HashMap::new()
    /// ).await?;
    ///
    /// assert_eq!(exit_code, 0);
    /// assert_eq!(stdout.trim(), "Hello, World!");
    /// ```
    ///
    /// # Async Behavior
    /// This method uses `tokio::task::spawn_blocking` to run the isolation
    /// logic in a separate thread, preventing blocking of the async runtime.
    ///
    /// # Security Notes
    /// - Commands run with no network access (isolated network namespace)
    /// - No access to host processes or IPC mechanisms
    /// - Limited filesystem access based on mount configuration
    /// - All security flags are applied to mounted filesystems
    pub async fn run_command(
        &self,
        cmd: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<(String, String, i32), ContainerError> {
        let cmd_string = cmd.to_string();
        let args_vec = args.to_vec();
        let env_map = env.clone();
        let request_id = self.request_id.clone();
        let root = self.root.clone();
        let work_dir = self.fs_cfg.work_dir.target.clone();

        // Run container execution in a blocking thread because:
        // 1. Linux system calls (fork, unshare, mount, exec) are inherently blocking
        // 2. These operations cannot be made async - they must complete synchronously
        // 3. Running blocking operations on the async runtime thread would starve other tasks
        // 4. spawn_blocking moves the work to a dedicated thread pool designed for blocking I/O
        // 5. The isolation process involves multiple fork() calls which create new processes
        //    that cannot be safely interrupted or made async
        tokio::task::spawn_blocking(move || {
            execution::run_isolated_blocking(
                &request_id,
                &root,
                &work_dir,
                &cmd_string,
                &args_vec,
                &env_map,
            )
        })
        .await
        .map_err(|e| ContainerError::Spawn {
            cmd: cmd.to_string(),
            source: std::io::Error::other(format!("Join error: {e}")),
        })?
    }

    // === Internal Helper Methods ===

    /// Creates the container root directory with secure permissions.
    ///
    /// This internal method creates the container's root directory and sets
    /// restrictive permissions (0o700) to ensure only the current user can
    /// access the container filesystem. This provides an additional layer
    /// of security beyond the namespace isolation.
    ///
    /// # Returns
    /// * `Ok(())` - Root directory created successfully
    /// * `Err(ContainerError)` - Directory creation or permission setting failed
    ///
    /// # Security
    /// The 0o700 permissions ensure that:
    /// - Only the owner can read, write, or execute
    /// - Other users and groups have no access
    /// - Container contents are protected at the filesystem level
    fn create_container_root(&self) -> Result<(), ContainerError> {
        debug!(
            "Creating container root for {} at {}",
            self.request_id,
            self.root.display()
        );
        fs::create_dir_all(&self.root).map_err(|e| ContainerError::CreateDir {
            path: self.root.clone(),
            source: e,
        })?;
        let mut perms = fs::metadata(&self.root)
            .map_err(|e| ContainerError::CreateDir {
                path: self.root.clone(),
                source: e,
            })?
            .permissions();

        let mode = 0o700u32;
        perms.set_mode(mode);
        fs::set_permissions(&self.root, perms).map_err(|e| ContainerError::SetPermissions {
            path: self.root.clone(),
            octal_mode: mode,
            source: e,
        })?;
        Ok(())
    }

    /// Sets up all container mounts by delegating to the mounts module.
    ///
    /// This internal method creates the base directories needed for tmpfs
    /// mounts (work and tmp directories) and then delegates to the mounts
    /// module to perform the actual mount operations.
    ///
    /// # Returns
    /// * `Ok(())` - All mounts set up successfully
    /// * `Err(ContainerError)` - Mount setup failed
    ///
    /// # Mount Order
    /// The mounts are set up in a specific order to handle dependencies:
    /// 1. Base directories for tmpfs mounts
    /// 2. Folder mounts (may contain submounts)
    /// 3. Tmpfs mounts (temporary filesystems)
    /// 4. Device mounts (controlled device access)
    /// 5. File mounts (individual files)
    /// 6. Essential filesystems (/proc, /sys)
    fn setup_mounts(&self) -> Result<(), ContainerError> {
        debug!("Setting up mounts for {}", self.root.display());

        // Ensure base directories exist for tmpfs mounts declared as work/tmp
        for rel in [
            self.fs_cfg.work_dir.target.as_str(),
            self.fs_cfg.tmp_dir.target.as_str(),
        ] {
            let dir = self.root.join(rel);
            fs::create_dir_all(&dir).map_err(|e| ContainerError::CreateDir {
                path: dir.clone(),
                source: e,
            })?;
        }

        mounts::setup_mounts(&self.root, &self.fs_cfg.mounts)
    }

    /// Unmounts all container filesystems by delegating to the mounts module.
    ///
    /// This internal method delegates the unmounting process to the mounts
    /// module, which handles the proper order and error handling for
    /// unmounting all container filesystems.
    ///
    /// # Returns
    /// * `Ok(())` - All filesystems unmounted successfully
    /// * `Err(ContainerError)` - Unmounting failed for at least one filesystem
    ///
    /// # Unmount Order
    /// Filesystems are unmounted in reverse order to handle dependencies:
    /// 1. Individual file mounts
    /// 2. Device mounts  
    /// 3. Tmpfs mounts
    /// 4. Folder mounts
    fn umount_all(&self) -> Result<(), ContainerError> {
        mounts::umount_all(&self.root, &self.fs_cfg.mounts)
    }
}
