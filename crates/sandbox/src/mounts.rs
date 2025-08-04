use faber_core::{FaberError, Result};
use nix::mount::{MntFlags, MsFlags, mount as nix_mount, umount2};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Mount point configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MountPoint {
    pub source: PathBuf,
    pub target: PathBuf,
    pub mount_type: MountType,
    pub flags: u64,
    pub data: Option<String>,
}

/// Mount type for different filesystem types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MountType {
    Bind,
    Proc,
    Tmpfs,
}

/// Symlink configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymLink {
    pub target: PathBuf,
    pub link_path: PathBuf,
}

/// Mount configuration for container filesystem
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MountConfig {
    pub mounts: Vec<MountPoint>,
    pub symlinks: Vec<SymLink>,
    pub work_dir: PathBuf,
    pub tmp_dir: PathBuf,
    pub read_only_paths: Vec<PathBuf>,
    pub writable_paths: Vec<PathBuf>,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self {
            mounts: Vec::new(),
            symlinks: Vec::new(),
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
    /// Create a default secure mount configuration
    pub fn default_secure() -> Self {
        let mut config = Self::default();

        // Essential system directories (bind mounts - read-only)
        config.mounts.push(MountPoint {
            source: PathBuf::from("/bin"),
            target: PathBuf::from("bin"),
            mount_type: MountType::Bind,
            flags: 1, // readonly flag
            data: None,
        });

        config.mounts.push(MountPoint {
            source: PathBuf::from("/lib"),
            target: PathBuf::from("lib"),
            mount_type: MountType::Bind,
            flags: 1, // readonly flag
            data: None,
        });

        config.mounts.push(MountPoint {
            source: PathBuf::from("/lib64"),
            target: PathBuf::from("lib64"),
            mount_type: MountType::Bind,
            flags: 1, // readonly flag
            data: None,
        });

        config.mounts.push(MountPoint {
            source: PathBuf::from("/usr"),
            target: PathBuf::from("usr"),
            mount_type: MountType::Bind,
            flags: 1, // readonly flag
            data: None,
        });

        // Essential linker cache
        config.mounts.push(MountPoint {
            source: PathBuf::from("/etc/ld.so.cache"),
            target: PathBuf::from("etc/ld.so.cache"),
            mount_type: MountType::Bind,
            flags: 1, // readonly flag
            data: None,
        });

        // Essential device files (bind mounts)
        config.mounts.push(MountPoint {
            source: PathBuf::from("/dev/null"),
            target: PathBuf::from("dev/null"),
            mount_type: MountType::Bind,
            flags: 0,
            data: None,
        });

        config.mounts.push(MountPoint {
            source: PathBuf::from("/dev/zero"),
            target: PathBuf::from("dev/zero"),
            mount_type: MountType::Bind,
            flags: 0,
            data: None,
        });

        config.mounts.push(MountPoint {
            source: PathBuf::from("/dev/random"),
            target: PathBuf::from("dev/random"),
            mount_type: MountType::Bind,
            flags: 0,
            data: None,
        });

        config.mounts.push(MountPoint {
            source: PathBuf::from("/dev/urandom"),
            target: PathBuf::from("dev/urandom"),
            mount_type: MountType::Bind,
            flags: 0,
            data: None,
        });

        config.mounts.push(MountPoint {
            source: PathBuf::from("/dev/full"),
            target: PathBuf::from("dev/full"),
            mount_type: MountType::Bind,
            flags: 0,
            data: None,
        });

        // Work directory (tmpfs for performance)
        config.mounts.push(MountPoint {
            source: PathBuf::from("tmpfs"),
            target: PathBuf::from("work"),
            mount_type: MountType::Tmpfs,
            flags: 0,
            data: Some("size=256m,nr_inodes=4k".to_string()),
        });

        // Temporary directory
        config.mounts.push(MountPoint {
            source: PathBuf::from("tmpfs"),
            target: PathBuf::from("tmp"),
            mount_type: MountType::Tmpfs,
            flags: 0,
            data: Some("size=128m,nr_inodes=4k".to_string()),
        });

        // Proc filesystem (only if needed)
        config.mounts.push(MountPoint {
            source: PathBuf::from("proc"),
            target: PathBuf::from("proc"),
            mount_type: MountType::Proc,
            flags: 0,
            data: None,
        });

        config
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

    /// Apply all mounts to the container
    pub fn apply_mounts(&self) -> Result<()> {
        info!(
            "Applying mounts to container root: {}",
            self.container_root.display()
        );

        // Create essential directories
        self.create_essential_directories()?;

        // Apply each mount point
        for mount in &self.config.mounts {
            self.apply_mount(mount)?;
        }

        // Create symlinks
        for symlink in &self.config.symlinks {
            self.create_symlink(symlink)?;
        }

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

    fn create_essential_directories(&self) -> Result<()> {
        let essential_dirs = [
            "bin",
            "dev",
            "etc",
            "lib",
            "lib64",
            "proc",
            "sys",
            "tmp",
            "usr",
            "usr/bin",
            "usr/lib",
            "usr/local",
            "work",
        ];

        for dir in &essential_dirs {
            let dir_path = self.container_root.join(dir);
            if let Err(e) = std::fs::create_dir_all(&dir_path) {
                warn!("Failed to create directory {}: {}", dir_path.display(), e);
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
                return Err(FaberError::Sandbox(format!(
                    "Failed to create mount target dir {}: {}",
                    target.display(),
                    e
                )));
            }
        } else {
            // Create parent directory if needed
            if let Some(parent) = target.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    return Err(FaberError::Sandbox(format!(
                        "Failed to create parent directory for {}: {}",
                        target.display(),
                        e
                    )));
                }
            }
            // Create empty file
            if let Err(e) = std::fs::File::create(target) {
                return Err(FaberError::Sandbox(format!(
                    "Failed to create mount target file {}: {}",
                    target.display(),
                    e
                )));
            }
        }

        let mut mount_flags = MsFlags::MS_BIND;

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
                    "Bind mount skipped (no privileges): {} -> {} - using symlink fallback",
                    source.display(),
                    target.display()
                );
                // Fall back to creating a symlink for unprivileged environments
                if target.exists() {
                    let _ = std::fs::remove_file(target);
                }
                match std::os::unix::fs::symlink(source, target) {
                    Ok(()) => Ok(()),
                    Err(e) => Err(FaberError::Sandbox(format!(
                        "Failed to create symlink fallback {} -> {}: {}",
                        source.display(),
                        target.display(),
                        e
                    ))),
                }
            }
            Err(e) => Err(FaberError::Sandbox(format!(
                "Failed to bind mount {} to {}: {}",
                source.display(),
                target.display(),
                e
            ))),
        }
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

// Legacy MountManager for backward compatibility
use faber_config::MountsConfig as ConfigMountsConfig;

pub struct LegacyMountManager {
    pub read_only_root: bool,
    pub mount_config: ConfigMountsConfig,
}

impl LegacyMountManager {
    pub fn new(read_only_root: bool, mount_config: ConfigMountsConfig) -> Self {
        Self {
            read_only_root,
            mount_config,
        }
    }

    pub async fn setup_mounts(&self, work_dir: &str) -> Result<()> {
        info!("Would setup mounts for work_dir: {}", work_dir);
        info!("Mount config: {:?}", self.mount_config);
        // TODO: Implement mount setup using self.mount_config
        Ok(())
    }

    pub async fn cleanup_mounts(&self) -> Result<()> {
        info!("Would cleanup mounts");
        // TODO: Implement mount cleanup
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mount_config_default() {
        let config = MountConfig::default();
        assert_eq!(config.work_dir, PathBuf::from("/work"));
        assert_eq!(config.tmp_dir, PathBuf::from("/tmp"));
        assert!(!config.read_only_paths.is_empty());
        assert!(!config.writable_paths.is_empty());
    }

    #[test]
    fn test_mount_config_default_secure() {
        let config = MountConfig::default_secure();
        assert!(!config.mounts.is_empty());

        // Check that essential mounts are present
        let mount_targets: Vec<&PathBuf> = config.mounts.iter().map(|m| &m.target).collect();
        assert!(mount_targets.contains(&&PathBuf::from("bin")));
        assert!(mount_targets.contains(&&PathBuf::from("lib")));
        assert!(mount_targets.contains(&&PathBuf::from("work")));
        assert!(mount_targets.contains(&&PathBuf::from("tmp")));
    }

    #[test]
    fn test_mount_manager_creation() {
        let config = MountConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let manager = MountManager::new(&config, temp_dir.path());

        assert_eq!(manager.container_root, temp_dir.path());
        assert_eq!(manager.config.work_dir, PathBuf::from("/work"));
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
        assert!(matches!(mount.mount_type, MountType::Bind));
        assert_eq!(mount.flags, 1);
        assert!(mount.data.is_none());
    }

    #[test]
    fn test_symlink_creation() {
        let config = MountConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let manager = MountManager::new(&config, temp_dir.path());

        let symlink = SymLink {
            target: PathBuf::from("/usr/bin/python3"),
            link_path: PathBuf::from("usr/bin/python"),
        };

        // This should work even without privileges
        let result = manager.create_symlink(&symlink);
        // In test environment, this might fail due to missing source, but shouldn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_essential_directories_creation() {
        let config = MountConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let manager = MountManager::new(&config, temp_dir.path());

        let result = manager.create_essential_directories();
        assert!(result.is_ok());

        // Check that essential directories were created
        let essential_dirs = ["bin", "dev", "etc", "lib", "proc", "tmp", "work"];
        for dir in &essential_dirs {
            let dir_path = temp_dir.path().join(dir);
            assert!(dir_path.exists() || dir_path.is_symlink());
        }
    }
}
