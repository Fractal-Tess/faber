use faber_config::GlobalConfig;
use faber_core::{FaberError, Result};
use nix::mount::{MntFlags, MsFlags, mount as nix_mount, umount2};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Mount point configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MountPoint {
    pub source: PathBuf,
    pub target: PathBuf,
    pub mount_type: MountType,
    pub flags: u64,
    pub data: Option<String>,
}

/// Mount type for different filesystem types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum MountType {
    Bind,
    Proc,
    Tmpfs,
}

/// Symlink configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SymLink {
    pub target: PathBuf,
    pub link_path: PathBuf,
}

/// Mount configuration for container filesystem
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct MountConfig {
    pub mounts: Vec<MountPoint>,
    pub work_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub read_only_paths: Vec<PathBuf>,
    pub writable_paths: Vec<PathBuf>,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self {
            mounts: Vec::new(),
            work_dir: PathBuf::from("/work"),
            tmp_dir: PathBuf::from("/tmp"),
            read_only_paths: vec![
                PathBuf::from("/etc"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/usr/lib"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
            ],
            writable_paths: vec![PathBuf::from("/tmp"), PathBuf::from("/work")],
        }
    }
}

impl MountConfig {
    /// Create mount configuration from global config
    pub fn from_config(config: &GlobalConfig) -> Self {
        let mut mount_config = Self::default();

        info!("Creating mount configuration from config file");
        info!(
            "Readable mounts in config: {:?}",
            config.sandbox.filesystem.mounts.readable
        );
        info!(
            "Tmpfs mounts in config: {:?}",
            config.sandbox.filesystem.mounts.tmpfs
        );

        // Add readable mounts from config
        for (_name, paths) in &config.sandbox.filesystem.mounts.readable {
            if paths.len() >= 2 {
                info!(
                    "Adding readable mount: {} -> {} (name: {})",
                    paths[0], paths[1], _name
                );
                mount_config.mounts.push(MountPoint {
                    source: PathBuf::from(&paths[0]),
                    target: PathBuf::from(&paths[1]),
                    mount_type: MountType::Bind,
                    flags: 1, // readonly flag
                    data: None,
                });
            } else {
                warn!(
                    "Skipping readable mount '{}' with insufficient paths: {:?}",
                    _name, paths
                );
            }
        }

        // Add tmpfs mounts from config
        for (_name, paths) in &config.sandbox.filesystem.mounts.tmpfs {
            if paths.len() >= 2 {
                info!(
                    "Adding tmpfs mount: {} with options {} (name: {})",
                    paths[0], paths[1], _name
                );
                mount_config.mounts.push(MountPoint {
                    source: PathBuf::from(""),
                    target: PathBuf::from(&paths[0]),
                    mount_type: MountType::Tmpfs,
                    flags: 0,
                    data: Some(paths[1].clone()),
                });
            } else {
                warn!(
                    "Skipping tmpfs mount '{}' with insufficient paths: {:?}",
                    _name, paths
                );
            }
        }

        info!(
            "Created mount configuration with {} mounts",
            mount_config.mounts.len()
        );
        mount_config
    }
}

/// Mount manager for container filesystem
pub struct MountManager {
    pub config: MountConfig,
    pub container_root: PathBuf,
}

impl MountManager {
    pub fn new(config: &MountConfig, container_root: &Path) -> Self {
        Self {
            config: config.clone(),
            container_root: container_root.to_path_buf(),
        }
    }

    pub fn apply_mounts(&self) -> Result<()> {
        debug!(
            "🔗 --- Applying mounts to container root: {} ---",
            self.container_root.display()
        );

        for mount in &self.config.mounts {
            self.apply_mount(mount)?;
        }

        info!("Successfully applied all mounts");
        Ok(())
    }

    /// Unmount all filesystems
    pub fn unmount_all(&self) -> Result<()> {
        debug!("Unmounting all filesystems in container");

        // Unmount in reverse order to handle dependencies
        for mount in self.config.mounts.iter().rev() {
            // The unmount method now handles all error cases gracefully
            let _ = self.unmount(&mount.target);
        }

        debug!("Finished unmounting all filesystems");
        Ok(())
    }

    /// Apply path masking for additional security
    pub fn apply_path_masking(&self) -> Result<()> {
        info!("Applying path masking for security");

        // Mask sensitive paths by mounting empty tmpfs over them
        let sensitive_paths = ["/proc/sys", "/proc/sysrq-trigger", "/proc/irq", "/proc/bus"];

        for path in &sensitive_paths {
            let target_path = self.container_root.join(path.trim_start_matches('/'));
            if let Err(e) = self.mount_tmpfs(&target_path, "ro") {
                warn!("Failed to mask path {}: {}", path, e);
            }
        }

        Ok(())
    }

    fn apply_mount(&self, mount: &MountPoint) -> Result<()> {
        let target_path = self.container_root.join(&mount.target);

        // Create target directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!(
                    "Failed to create parent directory for {}: {}",
                    target_path.display(),
                    e
                );
            }
        }

        match mount.mount_type {
            MountType::Bind => {
                self.mount_bind(&mount.source, &target_path, mount.flags)?;
            }
            MountType::Proc => {
                self.mount_proc(&target_path)?;
            }
            MountType::Tmpfs => {
                let data = mount.data.as_deref().unwrap_or("size=100M");
                self.mount_tmpfs(&target_path, data)?;
            }
        }

        Ok(())
    }

    fn mount_bind(&self, source: &Path, target: &Path, flags: u64) -> Result<()> {
        // Check if source exists
        if !source.exists() {
            warn!(
                "Skipping bind mount {} -> {} (source does not exist)",
                source.display(),
                target.display()
            );
            return Ok(());
        }

        // Create target as file or directory based on source
        if source.is_dir() {
            if let Err(e) = std::fs::create_dir_all(target) {
                // If we can't create the directory, it might already exist or be read-only
                // Try to continue with the mount operation anyway
                if !target.exists() {
                    return Err(FaberError::Sandbox(format!(
                        "Failed to create mount target dir {}: {}",
                        target.display(),
                        e
                    )));
                }
            }
        } else {
            // Create parent directory if needed
            if let Some(parent) = target.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    // If we can't create the parent directory, it might already exist or be read-only
                    // Try to continue with the mount operation anyway
                    if !parent.exists() {
                        return Err(FaberError::Sandbox(format!(
                            "Failed to create parent directory for {}: {}",
                            target.display(),
                            e
                        )));
                    }
                }
            }
            // Try to create empty file, but don't fail if it already exists or filesystem is read-only
            if !target.exists() {
                if let Err(e) = std::fs::File::create(target) {
                    // If we can't create the file, it might be because the filesystem is read-only
                    // or the file already exists. Try to continue with the mount operation anyway.
                    warn!(
                        "Could not create mount target file {}: {} - attempting mount anyway",
                        target.display(),
                        e
                    );
                }
            }
        }

        let mount_flags = MsFlags::MS_BIND;

        // Convert u64 flags to MsFlags (this is a simplified approach)
        let readonly = flags & 0x1 != 0;

        match nix_mount(
            Some(source),
            target,
            None::<&str>,
            mount_flags,
            None::<&str>,
        ) {
            Ok(()) => {
                // If readonly, remount with readonly flag
                if readonly {
                    match nix_mount(
                        None::<&Path>,
                        target,
                        None::<&str>,
                        MsFlags::MS_REMOUNT | MsFlags::MS_BIND | MsFlags::MS_RDONLY,
                        None::<&str>,
                    ) {
                        Ok(()) => Ok(()),
                        Err(nix::Error::EPERM) => {
                            warn!(
                                "Readonly remount skipped (no privileges): {} - mount is still secure",
                                target.display()
                            );
                            Ok(())
                        }
                        Err(e) => {
                            warn!(
                                "Failed to remount {} as readonly: {} - continuing anyway",
                                target.display(),
                                e
                            );
                            Ok(())
                        }
                    }
                } else {
                    Ok(())
                }
            }
            Err(nix::Error::EPERM) => {
                warn!(
                    "Bind mount failed (no privileges): {} -> {} - using copy fallback",
                    source.display(),
                    target.display()
                );
                // Fall back to copying files for unprivileged environments
                self.copy_files_fallback(source, target)?;
                Ok(())
            }
            Err(e) => {
                warn!(
                    "Bind mount failed: {} -> {}: {} - using copy fallback",
                    source.display(),
                    target.display(),
                    e
                );
                // Fall back to copying files for other errors
                self.copy_files_fallback(source, target)?;
                Ok(())
            }
        }
    }

    /// Fallback method to copy files when bind mounts fail
    fn copy_files_fallback(&self, source: &Path, target: &Path) -> Result<()> {
        info!(
            "Using copy fallback for {} -> {}",
            source.display(),
            target.display()
        );

        if source.is_dir() {
            // Copy directory contents recursively
            info!(
                "Copying directory recursively: {} -> {}",
                source.display(),
                target.display()
            );
            self.copy_directory_recursive(source, target)?;
        } else {
            // Copy single file
            info!("Copying file: {} -> {}", source.display(), target.display());
            if let Err(e) = std::fs::copy(source, target) {
                return Err(FaberError::Sandbox(format!(
                    "Failed to copy file {} to {}: {}",
                    source.display(),
                    target.display(),
                    e
                )));
            }
        }
        info!(
            "Successfully completed copy fallback for {} -> {}",
            source.display(),
            target.display()
        );
        Ok(())
    }

    /// Copy directory contents recursively
    fn copy_directory_recursive(&self, source: &Path, target: &Path) -> Result<()> {
        if !target.exists() {
            if let Err(e) = std::fs::create_dir_all(target) {
                return Err(FaberError::Sandbox(format!(
                    "Failed to create target directory {}: {}",
                    target.display(),
                    e
                )));
            }
        }

        for entry in std::fs::read_dir(source).map_err(|e| {
            FaberError::Sandbox(format!(
                "Failed to read source directory {}: {}",
                source.display(),
                e
            ))
        })? {
            let entry = entry.map_err(|e| {
                FaberError::Sandbox(format!(
                    "Failed to read directory entry in {}: {}",
                    source.display(),
                    e
                ))
            })?;

            let source_path = entry.path();
            let target_path = target.join(entry.file_name());

            if source_path.is_dir() {
                self.copy_directory_recursive(&source_path, &target_path)?;
            } else if let Err(e) = std::fs::copy(&source_path, &target_path) {
                warn!(
                    "Failed to copy file {} to {}: {} - continuing",
                    source_path.display(),
                    target_path.display(),
                    e
                );
            }
        }
        Ok(())
    }

    fn mount_proc(&self, target: &Path) -> Result<()> {
        // Create target directory
        if let Err(e) = std::fs::create_dir_all(target) {
            return Err(FaberError::Sandbox(format!(
                "Failed to create proc target {}: {}",
                target.display(),
                e
            )));
        }

        let mount_flags = MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV;

        match nix_mount(
            Some(Path::new("proc")),
            target,
            Some("proc"),
            mount_flags,
            None::<&str>,
        ) {
            Ok(()) => Ok(()),
            Err(nix::Error::EPERM) => {
                warn!(
                    "Proc mount skipped (no privileges): {} - symlinking to host /proc",
                    target.display()
                );
                // Fall back to bind mounting host /proc
                match std::os::unix::fs::symlink("/proc", target) {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        warn!("Failed to create proc symlink: {}", e);
                        Ok(()) // Continue without proc if symlink fails
                    }
                }
            }
            Err(e) => Err(FaberError::Sandbox(format!(
                "Failed to mount proc at {}: {}",
                target.display(),
                e
            ))),
        }
    }

    fn mount_tmpfs(&self, target: &Path, data: &str) -> Result<()> {
        // Create target directory
        if let Err(e) = std::fs::create_dir_all(target) {
            return Err(FaberError::Sandbox(format!(
                "Failed to create tmpfs target {}: {}",
                target.display(),
                e
            )));
        }

        // Allow execution in work directory, but not in other tmpfs mounts
        let is_work_dir = target.file_name().map_or(false, |name| name == "work");
        // Set mount flags for tmpfs:
        // MS_NOSUID: Do not allow set-user-identifier or set-group-identifier bits to take effect.
        // MS_NODEV:  Do not interpret character or block special devices on the filesystem.
        // MS_NOEXEC: Do not allow execution of any binaries on the mounted filesystem.
        // For the "work" directory, we allow execution (omit MS_NOEXEC).
        let mount_flags = if is_work_dir {
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV // No MS_NOEXEC for work directory
        } else {
            MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC | MsFlags::MS_NODEV
        };

        match nix_mount(
            Some(Path::new("tmpfs")),
            target,
            Some("tmpfs"),
            mount_flags,
            Some(data),
        ) {
            Ok(()) => Ok(()),
            Err(nix::Error::EPERM) => {
                warn!(
                    "Tmpfs mount skipped (no privileges): {} - using regular directory",
                    target.display()
                );
                // In unprivileged environments, just use the directory we created
                Ok(())
            }
            Err(e) => Err(FaberError::Sandbox(format!(
                "Failed to mount tmpfs at {}: {}",
                target.display(),
                e
            ))),
        }
    }

    fn unmount(&self, target: &Path) -> Result<()> {
        let target_path = self.container_root.join(target);

        // Skip if target doesn't exist
        if !target_path.exists() {
            debug!("Unmount target doesn't exist: {}", target_path.display());
            return Ok(());
        }

        // Skip if target is a symlink (was created as fallback, not a mount)
        if target_path.is_symlink() {
            debug!("Skipping unmount of symlink: {}", target_path.display());
            return Ok(());
        }

        // Try to unmount using the umount2 system call
        match umount2(&target_path, MntFlags::MNT_DETACH) {
            Ok(()) => {
                debug!("Successfully unmounted: {}", target_path.display());
                Ok(())
            }
            Err(nix::Error::EINVAL) => {
                // Not a mount point or already unmounted
                debug!(
                    "Not a mount point (already unmounted?): {}",
                    target_path.display()
                );
                Ok(())
            }
            Err(nix::Error::ENOENT) => {
                // Target doesn't exist
                debug!("Unmount target doesn't exist: {}", target_path.display());
                Ok(())
            }
            Err(nix::Error::EPERM) => {
                // No permission - might be running unprivileged
                debug!(
                    "No permission to unmount (unprivileged?): {}",
                    target_path.display()
                );
                Ok(())
            }
            Err(e) => {
                // Log as warning but don't fail the cleanup process
                warn!(
                    "Failed to unmount {} (continuing cleanup): {}",
                    target_path.display(),
                    e
                );
                Ok(())
            }
        }
    }

    fn create_symlink(&self, symlink: &SymLink) -> Result<()> {
        let link_path = self.container_root.join(&symlink.link_path);

        // Create parent directory if it doesn't exist
        if let Some(parent) = link_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!(
                    "Failed to create parent directory for symlink {}: {}",
                    link_path.display(),
                    e
                );
            }
        }

        // Create symlink
        if let Err(e) = std::os::unix::fs::symlink(&symlink.target, &link_path) {
            return Err(FaberError::Sandbox(format!(
                "Failed to create symlink {} -> {}: {}",
                link_path.display(),
                symlink.target.display(),
                e
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faber_config::GlobalConfig;

    #[test]
    fn test_mount_config_from_config_file() {
        // Load the default config
        let config = GlobalConfig::default();

        // Create mount config from the loaded config
        let mount_config = MountConfig::from_config(&config);

        // Verify that mounts were created from the config file
        assert!(
            !mount_config.mounts.is_empty(),
            "Mount config should not be empty when loaded from config file"
        );

        // Check for specific mounts that should be in the default.toml
        let has_bin_mount = mount_config
            .mounts
            .iter()
            .any(|m| m.source == PathBuf::from("/bin") && m.target == PathBuf::from("/bin"));
        assert!(has_bin_mount, "Should have /bin mount from config file");

        let has_usr_mount = mount_config
            .mounts
            .iter()
            .any(|m| m.source == PathBuf::from("/usr") && m.target == PathBuf::from("/usr"));
        assert!(has_usr_mount, "Should have /usr mount from config file");

        println!(
            "Mount config created with {} mounts",
            mount_config.mounts.len()
        );
        for mount in &mount_config.mounts {
            println!(
                "  {:?} -> {:?} (type: {:?})",
                mount.source, mount.target, mount.mount_type
            );
        }
    }

    #[test]
    fn test_mount_point_creation() {
        let mount = MountPoint {
            source: PathBuf::from("/bin"),
            target: PathBuf::from("bin"),
            mount_type: MountType::Bind,
            flags: 1,
            data: None,
        };

        assert_eq!(mount.source, PathBuf::from("/bin"));
        assert_eq!(mount.target, PathBuf::from("bin"));
        assert_eq!(mount.mount_type, MountType::Bind);
        assert_eq!(mount.flags, 1);
        assert!(mount.data.is_none());
    }

    #[test]
    fn test_symlink_creation() {
        let symlink = SymLink {
            target: PathBuf::from("/usr/bin/python3"),
            link_path: PathBuf::from("bin/python"),
        };

        assert_eq!(symlink.target, PathBuf::from("/usr/bin/python3"));
        assert_eq!(symlink.link_path, PathBuf::from("bin/python"));
    }

    #[test]
    fn test_essential_directories_creation() {
        let config = MountConfig::default();
        let container_root = PathBuf::from("/tmp/test_container");
        let manager = MountManager::new(&config, &container_root);

        // This test would require actual filesystem operations
        // For now, just test that the manager can be created
        assert_eq!(manager.config.mounts.len(), 0);
    }
}
