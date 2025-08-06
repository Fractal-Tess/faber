use nix::NixPath;
use nix::mount::{MsFlags, mount as nix_mount, umount};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, error, info, warn};

use crate::config::FilesystemConfig;

#[derive(Debug, thiserror::Error)]
pub enum FilesystemError {
    #[error(transparent)]
    MountError(nix::errno::Errno),
    #[error(transparent)]
    UnmountError(nix::Error),
    #[error("Failed to create directory: {0}")]
    DirectoryCreation(#[from] std::io::Error),
    #[error("Failed to create device: {0}")]
    DeviceCreation(String),
}

/// Container filesystem manager
pub struct ContainerFilesystem {
    container_path: PathBuf,
    config: FilesystemConfig,
    mounted_paths: Vec<PathBuf>,
}

impl ContainerFilesystem {
    pub fn new(container_path: PathBuf, config: FilesystemConfig) -> Self {
        debug!(
            "Creating ContainerFilesystem with path: {:?}",
            container_path
        );
        debug!("Filesystem config: {:?}", config);
        Self {
            container_path,
            config,
            mounted_paths: Vec::new(),
        }
    }

    /// Initialize the filesystem with all configured mounts
    pub async fn initialize(&mut self) -> Result<(), FilesystemError> {
        info!("Initializing container filesystem");
        debug!("Container base path: {:?}", self.container_path);
        debug!("Container path exists: {}", self.container_path.exists());
        debug!(
            "Container path is directory: {}",
            self.container_path.is_dir()
        );

        // Ensure container base directory exists
        if !self.container_path.exists() {
            debug!(
                "Creating container base directory: {:?}",
                self.container_path
            );
            fs::create_dir_all(&self.container_path).await?;
            debug!("Container base directory created successfully");
        }

        // Mount folders (read-only or read-write)
        debug!(
            "Processing {} folder mounts",
            self.config.mounts.folders.len()
        );
        let folder_mounts = self.config.mounts.folders.clone();
        for (i, mount) in folder_mounts.iter().enumerate() {
            debug!("Processing folder mount {}: {:?}", i, mount);
            if let Err(e) = self.mount_folder(mount).await {
                warn!("Failed to mount folder {}: {}, continuing...", i, e);
            }
        }

        // Mount tmpfs directories
        debug!("Processing {} tmpfs mounts", self.config.mounts.tmpfs.len());
        let tmpfs_mounts = self.config.mounts.tmpfs.clone();
        for (i, mount) in tmpfs_mounts.iter().enumerate() {
            debug!("Processing tmpfs mount {}: {:?}", i, mount);
            if let Err(e) = self.mount_tmpfs(mount).await {
                warn!("Failed to mount tmpfs {}: {}, continuing...", i, e);
            }
        }

        // Mount device files
        debug!(
            "Processing {} device mounts",
            self.config.mounts.devices.len()
        );
        let device_mounts = self.config.mounts.devices.clone();
        for (i, mount) in device_mounts.iter().enumerate() {
            debug!("Processing device mount {}: {:?}", i, mount);
            if let Err(e) = self.mount_device(mount).await {
                warn!("Failed to mount device {}: {}, continuing...", i, e);
            }
        }

        // Mount individual files
        debug!("Processing {} file mounts", self.config.mounts.files.len());
        let file_mounts = self.config.mounts.files.clone();
        for (i, mount) in file_mounts.iter().enumerate() {
            debug!("Processing file mount {}: {:?}", i, mount);
            if let Err(e) = self.mount_file(mount).await {
                warn!("Failed to mount file {}: {}, continuing...", i, e);
            }
        }

        // Create essential devices
        self.create_essential_devices().await?;

        // Create container configuration files
        self.create_container_config_files().await?;

        // Create symlinks for standard I/O
        self.create_standard_symlinks().await?;

        // Setup proc filesystem
        self.setup_proc_filesystem().await?;

        info!("Container filesystem initialized successfully");
        debug!("Total mounted paths: {:?}", self.mounted_paths);
        Ok(())
    }

    /// Cleanup all mounts
    pub async fn cleanup(&mut self) -> Result<(), FilesystemError> {
        info!("Cleaning up container filesystem");

        // Unmount all mounted paths in reverse order
        for path in self.mounted_paths.iter().rev() {
            if let Err(e) = self.unmount_path(path).await {
                // Only log as warning since unmount_path now handles most errors gracefully
                warn!("Failed to unmount {:?}: {}", path, e);
            }
        }

        self.mounted_paths.clear();
        info!("Container filesystem cleanup completed");
        Ok(())
    }

    /// Create essential device files in the container
    async fn create_essential_devices(&self) -> Result<(), FilesystemError> {
        debug!("Creating essential device files");

        let dev_path = self.container_path.join("dev");
        fs::create_dir_all(&dev_path).await?;

        // Essential devices: null, zero, random, urandom
        let devices = [
            ("null", nix::sys::stat::SFlag::S_IFCHR, 1, 3),
            ("zero", nix::sys::stat::SFlag::S_IFCHR, 1, 5),
            ("random", nix::sys::stat::SFlag::S_IFCHR, 1, 8),
            ("urandom", nix::sys::stat::SFlag::S_IFCHR, 1, 9),
        ];

        for (name, dev_type, major, minor) in &devices {
            let device_path = dev_path.join(name);

            // Check if device already exists (e.g., from filesystem mounting)
            if device_path.exists() {
                debug!("Device {} already exists, skipping creation", name);
                continue;
            }

            let dev = nix::sys::stat::makedev(*major, *minor);

            match nix::sys::stat::mknod(
                &device_path,
                *dev_type,
                nix::sys::stat::Mode::from_bits_truncate(0o666),
                dev,
            ) {
                Ok(_) => {
                    debug!("Successfully created device {}", name);
                }
                Err(nix::errno::Errno::EEXIST) => {
                    debug!("Device {} already exists (EEXIST), skipping", name);
                }
                Err(e) => {
                    return Err(FilesystemError::DeviceCreation(format!(
                        "Failed to create device {name}: {e}"
                    )));
                }
            }
        }

        debug!("Essential devices creation completed");
        Ok(())
    }

    /// Create standard symlinks for /dev/fd, /dev/stdin, /dev/stdout, /dev/stderr
    async fn create_standard_symlinks(&self) -> Result<(), FilesystemError> {
        debug!("Creating standard symlinks");

        let dev_path = self.container_path.join("dev");
        fs::create_dir_all(&dev_path).await?;

        // Create symlinks for standard I/O (matching go-judge)
        let symlinks = [
            ("fd", "/proc/self/fd"),
            ("stdin", "/proc/self/fd/0"),
            ("stdout", "/proc/self/fd/1"),
            ("stderr", "/proc/self/fd/2"),
        ];

        for (name, target) in &symlinks {
            let link_path = dev_path.join(name);

            // Remove existing symlink if it exists
            if link_path.exists() {
                fs::remove_file(&link_path).await?;
            }

            // Create symlink
            match std::os::unix::fs::symlink(target, &link_path) {
                Ok(_) => {
                    debug!("Successfully created symlink {} -> {}", name, target);
                }
                Err(e) => {
                    warn!("Failed to create symlink {} -> {}: {}", name, target, e);
                    // Continue with other symlinks
                }
            }
        }

        debug!("Standard symlinks creation completed");
        Ok(())
    }

    /// Setup proc filesystem
    async fn setup_proc_filesystem(&mut self) -> Result<(), FilesystemError> {
        debug!("Setting up proc filesystem");

        let proc_path = self.container_path.join("proc");

        // Create proc directory if it doesn't exist
        if !proc_path.exists() {
            fs::create_dir_all(&proc_path).await?;
        }

        // Mount proc filesystem
        let source = "proc";
        let target = proc_path.to_string_lossy();
        let fstype = "proc";
        let flags = MsFlags::empty();
        let data = "";

        match nix_mount(
            Some(source),
            target.as_ref(),
            Some(fstype),
            flags,
            Some(data),
        ) {
            Ok(_) => {
                debug!("Successfully mounted proc filesystem at {}", target);
                self.mounted_paths.push(proc_path);
            }
            Err(e) => {
                warn!("Failed to mount proc filesystem: {}", e);
                // Continue without proc filesystem
            }
        }

        debug!("Proc filesystem setup completed");
        Ok(())
    }

    /// Create basic container configuration files
    async fn create_container_config_files(&self) -> Result<(), FilesystemError> {
        debug!("Creating container configuration files");

        let etc_path = self.container_path.join("etc");
        fs::create_dir_all(&etc_path).await?;

        // Create passwd file (matching go-judge UID/GID 1536)
        let passwd_content =
            "root:x:0:0:root:/:/bin/sh\nuser:x:1536:1536:Container User:/w:/bin/sh\n";
        fs::write(etc_path.join("passwd"), passwd_content).await?;

        // Create group file (matching go-judge UID/GID 1536)
        let group_content = "root:x:0:\nuser:x:1536:\n";
        fs::write(etc_path.join("group"), group_content).await?;

        // Create hostname file
        fs::write(etc_path.join("hostname"), "faber-container\n").await?;

        debug!("Container configuration files created successfully");
        Ok(())
    }

    /// Mount a folder (read-only or read-write)
    async fn mount_folder(
        &mut self,
        mount: &crate::config::ReadOnlyMount,
    ) -> Result<(), FilesystemError> {
        debug!("=== Starting folder mount operation ===");
        debug!("Mount config: {:?}", mount);

        let target_path = self.container_path.join(&mount.target);
        debug!("Target path: {:?}", target_path);
        debug!("Target path exists: {}", target_path.exists());
        debug!("Target path is directory: {}", target_path.is_dir());

        // Create target directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            debug!("Parent directory: {:?}", parent);
            debug!("Parent exists: {}", parent.exists());
            debug!("Parent is directory: {}", parent.is_dir());

            if !parent.exists() {
                debug!("Creating parent directory: {:?}", parent);
                fs::create_dir_all(parent).await?;
                debug!("Parent directory created successfully");
            }
        }

        // Create the target directory itself if it doesn't exist
        if !target_path.exists() {
            debug!("Creating target directory: {:?}", target_path);
            fs::create_dir_all(&target_path).await?;
            debug!("Target directory created successfully");
        }

        debug!("Mounting folder: {} -> {:?}", mount.source, target_path);

        // Check if source exists
        let source_path = std::path::Path::new(&mount.source);
        debug!("Source path: {:?}", source_path);
        debug!("Source exists: {}", source_path.exists());
        debug!("Source is directory: {}", source_path.is_dir());
        debug!("Source is file: {}", source_path.is_file());

        if !source_path.exists() {
            warn!(
                "Source path {} does not exist, skipping mount",
                mount.source
            );
            // Don't add to mounted_paths since we didn't actually mount anything
            return Ok(());
        }

        // Mount with bind flag
        debug!("Attempting to mount with bind flag...");
        let mount_result = nix_mount(
            Some(mount.source.as_str()),
            target_path.as_os_str(),
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        );

        match mount_result {
            Ok(_) => {
                debug!("Successfully mounted {} to {:?}", mount.source, target_path);

                // Make it read-only if specified
                if matches!(
                    mount.permissions,
                    crate::config::FolderPermissions::ReadOnly
                ) {
                    debug!("Attempting to make folder read-only...");
                    let remount_result = nix_mount(
                        None::<&str>,
                        target_path.as_os_str(),
                        None::<&str>,
                        MsFlags::MS_BIND | MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT,
                        None::<&str>,
                    );

                    match remount_result {
                        Ok(_) => debug!("Successfully made {} read-only", mount.source),
                        Err(e) => {
                            warn!("Failed to make {} read-only: {}", mount.source, e);
                            // Continue anyway, the mount is still functional
                        }
                    }
                }

                // Only add to mounted_paths if the mount was successful
                self.mounted_paths.push(target_path);
            }
            Err(e) => {
                error!(
                    "Failed to mount {} to {:?}: {}",
                    mount.source, target_path, e
                );
                return Err(FilesystemError::MountError(e));
            }
        }

        Ok(())
    }

    /// Mount a tmpfs directory
    async fn mount_tmpfs(
        &mut self,
        mount: &crate::config::TempfsMount,
    ) -> Result<(), FilesystemError> {
        debug!("=== Starting tmpfs mount operation ===");
        debug!("Mount config: {:?}", mount);

        let target_path = self.container_path.join(&mount.target);
        debug!("Target path: {:?}", target_path);
        debug!("Target path exists: {}", target_path.exists());
        debug!("Target path is directory: {}", target_path.is_dir());

        // Create target directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            debug!("Parent directory: {:?}", parent);
            debug!("Parent exists: {}", parent.exists());
            debug!("Parent is directory: {}", parent.is_dir());

            if !parent.exists() {
                debug!("Creating parent directory: {:?}", parent);
                fs::create_dir_all(parent).await?;
                debug!("Parent directory created successfully");
            }
        }

        // Create the target directory itself if it doesn't exist
        if !target_path.exists() {
            debug!("Creating target directory: {:?}", target_path);
            fs::create_dir_all(&target_path).await?;
            debug!("Target directory created successfully");
        }

        debug!(
            "Mounting tmpfs: {} -> {:?} with options: {}",
            mount.target, target_path, mount.options
        );

        // Mount tmpfs
        debug!("Attempting to mount tmpfs...");
        let mount_result = nix_mount(
            Some("tmpfs"),
            target_path.as_os_str(),
            Some("tmpfs"),
            MsFlags::empty(),
            Some(mount.options.as_str()),
        );

        match mount_result {
            Ok(_) => debug!("Successfully mounted tmpfs to {:?}", target_path),
            Err(e) => {
                error!("Failed to mount tmpfs to {:?}: {}", target_path, e);
                return Err(FilesystemError::MountError(e));
            }
        }

        self.mounted_paths.push(target_path);
        debug!("=== Completed tmpfs mount operation ===");
        Ok(())
    }

    /// Mount a device file
    async fn mount_device(
        &mut self,
        mount: &crate::config::DeviceMount,
    ) -> Result<(), FilesystemError> {
        debug!("=== Starting device mount operation ===");
        debug!("Mount config: {:?}", mount);

        let target_path = self.container_path.join(&mount.target);
        debug!("Target path: {:?}", target_path);
        debug!("Target path exists: {}", target_path.exists());
        debug!("Target path is directory: {}", target_path.is_dir());

        // Create parent directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            debug!("Parent directory: {:?}", parent);
            debug!("Parent exists: {}", parent.exists());
            debug!("Parent is directory: {}", parent.is_dir());

            if !parent.exists() {
                debug!("Creating parent directory: {:?}", parent);
                fs::create_dir_all(parent).await?;
                debug!("Parent directory created successfully");
            }
        }

        debug!(
            "Mounting device: {} -> {:?} ({:?})",
            mount.source, target_path, mount.permissions
        );

        // Check if source exists
        let source_path = std::path::Path::new(&mount.source);
        debug!("Source path: {:?}", source_path);
        debug!("Source exists: {}", source_path.exists());
        debug!("Source is file: {}", source_path.is_file());
        debug!("Source is directory: {}", source_path.is_dir());

        if !source_path.exists() {
            warn!(
                "Source device {} does not exist, skipping mount",
                mount.source
            );
            return Ok(());
        }

        // Check if target already exists and remove it if it's a file
        if target_path.exists() && target_path.is_file() {
            debug!("Removing existing target file: {:?}", target_path);
            if let Err(e) = std::fs::remove_file(&target_path) {
                warn!(
                    "Failed to remove existing target file {:?}: {}",
                    target_path, e
                );
            }
        }

        // Create the target file if it doesn't exist (required for bind mount)
        if !target_path.exists() {
            debug!("Creating target device file: {:?}", target_path);
            // Create an empty file that will be replaced by the bind mount
            if let Err(e) = std::fs::File::create(&target_path) {
                error!(
                    "Failed to create target device file {:?}: {}",
                    target_path, e
                );
                return Err(FilesystemError::DirectoryCreation(e));
            }
            debug!("Target device file created successfully");
        }

        // Mount with bind flag
        debug!("Attempting to mount device with bind flag...");
        let mount_result = nix_mount(
            Some(mount.source.as_str()),
            target_path.as_os_str(),
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        );

        match mount_result {
            Ok(_) => {
                debug!(
                    "Successfully mounted device {} to {:?}",
                    mount.source, target_path
                );

                // Make it read-only if specified
                if matches!(
                    mount.permissions,
                    crate::config::DevicePermissions::ReadOnly
                ) {
                    debug!("Attempting to make device read-only...");
                    let remount_result = nix_mount(
                        None::<&str>,
                        target_path.as_os_str(),
                        None::<&str>,
                        MsFlags::MS_BIND | MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT,
                        None::<&str>,
                    );

                    match remount_result {
                        Ok(_) => debug!("Successfully made device {} read-only", mount.source),
                        Err(e) => {
                            warn!("Failed to make device {} read-only: {}", mount.source, e);
                            // Continue anyway, the mount is still functional
                        }
                    }
                }

                // Only add to mounted_paths if the mount was successful
                self.mounted_paths.push(target_path);
            }
            Err(e) => {
                error!(
                    "Failed to mount device {} to {:?}: {}",
                    mount.source, target_path, e
                );
                return Err(FilesystemError::MountError(e));
            }
        }

        Ok(())
    }

    /// Mount an individual file
    async fn mount_file(
        &mut self,
        mount: &crate::config::FileMount,
    ) -> Result<(), FilesystemError> {
        debug!("=== Starting file mount operation ===");
        debug!("Mount config: {:?}", mount);

        let target_path = self.container_path.join(&mount.target);
        debug!("Target path: {:?}", target_path);
        debug!("Target path exists: {}", target_path.exists());
        debug!("Target path is directory: {}", target_path.is_dir());

        // Check if source exists
        let source_path = std::path::Path::new(&mount.source);
        debug!("Source path: {:?}", source_path);
        debug!("Source exists: {}", source_path.exists());
        debug!("Source is file: {}", source_path.is_file());
        debug!("Source is directory: {}", source_path.is_dir());

        if !source_path.exists() {
            warn!(
                "Source file {} does not exist, skipping mount",
                mount.source
            );
            return Ok(());
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            debug!("Parent directory: {:?}", parent);
            debug!("Parent exists: {}", parent.exists());
            debug!("Parent is directory: {}", parent.is_dir());

            if !parent.exists() {
                debug!("Creating parent directory: {:?}", parent);
                fs::create_dir_all(parent).await?;
                debug!("Parent directory created successfully");
            }
        }

        // Remove target if it exists (whether file or directory)
        if target_path.exists() {
            debug!("Target exists, removing: {:?}", target_path);
            if target_path.is_dir() {
                fs::remove_dir_all(&target_path).await?;
            } else {
                fs::remove_file(&target_path).await?;
            }
            debug!("Target removed successfully");
        }

        // Create the target file if it doesn't exist (required for bind mount)
        if !target_path.exists() {
            debug!("Creating target file: {:?}", target_path);
            // Create an empty file that will be replaced by the bind mount
            if let Err(e) = std::fs::File::create(&target_path) {
                error!("Failed to create target file {:?}: {}", target_path, e);
                return Err(FilesystemError::DirectoryCreation(e));
            }
            debug!("Target file created successfully");
        }

        debug!("Mounting file: {} -> {:?}", mount.source, target_path);

        // Mount with bind flag
        debug!("Attempting to mount file with bind flag...");
        let mount_result = nix_mount(
            Some(mount.source.as_str()),
            target_path.as_os_str(),
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        );

        match mount_result {
            Ok(_) => {
                debug!(
                    "Successfully mounted file {} to {:?}",
                    mount.source, target_path
                );

                // Make it read-only if specified
                if matches!(mount.permissions, crate::config::FilePermissions::ReadOnly) {
                    debug!("Attempting to make file read-only...");
                    let remount_result = nix_mount(
                        None::<&str>,
                        target_path.as_os_str(),
                        None::<&str>,
                        MsFlags::MS_BIND | MsFlags::MS_RDONLY | MsFlags::MS_REMOUNT,
                        None::<&str>,
                    );

                    match remount_result {
                        Ok(_) => debug!("Successfully made file {} read-only", mount.source),
                        Err(e) => {
                            warn!("Failed to make file {} read-only: {}", mount.source, e);
                            // Continue anyway, the mount is still functional
                        }
                    }
                }

                // Only add to mounted_paths if the mount was successful
                self.mounted_paths.push(target_path);
            }
            Err(e) => {
                error!(
                    "Failed to mount file {} to {:?}: {}",
                    mount.source, target_path, e
                );
                return Err(FilesystemError::MountError(e));
            }
        }

        Ok(())
    }

    /// Unmount a path
    async fn unmount_path(&self, path: &PathBuf) -> Result<(), FilesystemError> {
        debug!("Unmounting: {:?}", path);

        // Check if the path exists before trying to unmount
        if !path.exists() {
            debug!("Path {:?} does not exist, skipping unmount", path);
            return Ok(());
        }

        match umount(path.as_os_str()) {
            Ok(_) => {
                debug!("Successfully unmounted: {:?}", path);
                Ok(())
            }
            Err(nix::errno::Errno::ENOENT) => {
                debug!("Mount point {:?} does not exist (ENOENT), skipping", path);
                Ok(())
            }
            Err(nix::errno::Errno::EINVAL) => {
                debug!("Path {:?} is not a mount point (EINVAL), skipping", path);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to unmount {:?}: {}", path, e);
                // Don't return error for unmount failures during cleanup
                Ok(())
            }
        }
    }
}
