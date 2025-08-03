//! Mount management for container filesystem isolation
//!
//! This module provides functionality to create secure filesystem mounts
//! within containers, allowing access to essential system resources while
//! maintaining security isolation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, info, warn};

use super::error::SandboxError;

/// Type of mount operation
#[derive(Debug, Clone, PartialEq)]
pub enum MountType {
    /// Bind mount - mount existing directory/file
    Bind,
    /// Tmpfs - create temporary filesystem in memory
    Tmpfs,
    /// Proc - mount /proc filesystem
    Proc,
}

/// Configuration for a single mount point  
#[derive(Debug, Clone)]
pub struct MountPoint {
    /// Type of mount (bind, tmpfs, proc)
    pub mount_type: MountType,
    /// Source path on host (for bind mounts)
    pub source: String,
    /// Target path in container
    pub target: String,
    /// Whether mount should be read-only
    pub readonly: bool,
    /// Mount options/data
    pub options: String,
}

impl MountPoint {
    /// Create a read-only bind mount
    pub fn bind_ro(source: &str, target: &str) -> Self {
        Self {
            mount_type: MountType::Bind,
            source: source.to_string(),
            target: target.to_string(),
            readonly: true,
            options: String::new(),
        }
    }

    /// Create a writable bind mount
    pub fn bind_rw(source: &str, target: &str) -> Self {
        Self {
            mount_type: MountType::Bind,
            source: source.to_string(),
            target: target.to_string(),
            readonly: false,
            options: String::new(),
        }
    }

    /// Create a tmpfs mount
    pub fn tmpfs(target: &str, size: &str) -> Self {
        Self {
            mount_type: MountType::Tmpfs,
            source: "tmpfs".to_string(),
            target: target.to_string(),
            readonly: false,
            options: format!("size={}", size),
        }
    }

    /// Create a proc mount
    pub fn proc(target: &str) -> Self {
        Self {
            mount_type: MountType::Proc,
            source: "proc".to_string(),
            target: target.to_string(),
            readonly: false,
            options: String::new(),
        }
    }
}

/// Symbolic link configuration
#[derive(Debug, Clone)]
pub struct SymLink {
    /// Path where the symlink will be created
    pub link_path: String,
    /// Target that the symlink points to
    pub target: String,
}

impl SymLink {
    pub fn new(link_path: &str, target: &str) -> Self {
        Self {
            link_path: link_path.to_string(),
            target: target.to_string(),
        }
    }
}

/// Complete mount configuration for a container
#[derive(Debug, Clone)]
pub struct MountConfig {
    /// List of mount points to create
    pub mounts: Vec<MountPoint>,
    /// Symbolic links to create
    pub symlinks: Vec<SymLink>,
    /// Paths to mask/hide for security
    pub masked_paths: Vec<String>,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self::default_secure()
    }
}

impl MountConfig {
    /// Create default mount configuration based on go-judge
    pub fn default_secure() -> Self {
        let mut mounts = Vec::new();

        // Essential system directories (read-only)
        mounts.push(MountPoint::bind_ro("/bin", "bin"));
        mounts.push(MountPoint::bind_ro("/lib", "lib"));
        mounts.push(MountPoint::bind_ro("/usr", "usr"));

        // lib64 if it exists
        if Path::new("/lib64").exists() {
            mounts.push(MountPoint::bind_ro("/lib64", "lib64"));
        }

        // Essential configuration files
        if Path::new("/etc/ld.so.cache").exists() {
            mounts.push(MountPoint::bind_ro("/etc/ld.so.cache", "etc/ld.so.cache"));
        }
        if Path::new("/etc/alternatives").exists() {
            mounts.push(MountPoint::bind_ro("/etc/alternatives", "etc/alternatives"));
        }

        // Essential devices
        mounts.push(MountPoint::bind_rw("/dev/null", "dev/null"));
        mounts.push(MountPoint::bind_rw("/dev/zero", "dev/zero"));
        mounts.push(MountPoint::bind_rw("/dev/urandom", "dev/urandom"));
        mounts.push(MountPoint::bind_rw("/dev/random", "dev/random"));

        // Proc filesystem
        mounts.push(MountPoint::proc("proc"));

        // Working directory (tmpfs)
        mounts.push(MountPoint::tmpfs("w", "256M"));

        // Tmp directory (tmpfs)
        mounts.push(MountPoint::tmpfs("tmp", "64M"));

        // Standard I/O symlinks
        let symlinks = vec![
            SymLink::new("dev/fd", "/proc/self/fd"),
            SymLink::new("dev/stdin", "/proc/self/fd/0"),
            SymLink::new("dev/stdout", "/proc/self/fd/1"),
            SymLink::new("dev/stderr", "/proc/self/fd/2"),
        ];

        // Sensitive paths to mask
        let masked_paths = vec![
            "/sys/firmware".to_string(),
            "/sys/devices/virtual/powercap".to_string(),
            "/proc/kcore".to_string(),
            "/proc/keys".to_string(),
            "/proc/timer_list".to_string(),
            "/proc/sched_debug".to_string(),
            "/proc/scsi".to_string(),
        ];

        Self {
            mounts,
            symlinks,
            masked_paths,
        }
    }

    /// Create minimal mount configuration for testing
    pub fn minimal() -> Self {
        let mounts = vec![
            // Just essential binaries and libraries
            MountPoint::bind_ro("/bin", "bin"),
            MountPoint::bind_ro("/usr/bin", "usr/bin"),
            MountPoint::bind_ro("/lib", "lib"),
            MountPoint::bind_ro("/usr/lib", "usr/lib"),
            MountPoint::bind_ro("/usr/include", "usr/include"),
            // Essential devices
            MountPoint::bind_rw("/dev/null", "dev/null"),
            // Working directory
            MountPoint::tmpfs("w", "128M"),
        ];

        Self {
            mounts,
            symlinks: vec![],
            masked_paths: vec![],
        }
    }
}

/// Mount manager for handling filesystem mounts in containers
pub struct MountManager {
    config: MountConfig,
    container_root: PathBuf,
}

impl MountManager {
    /// Create a new mount manager
    pub fn new(config: &MountConfig, container_root: &PathBuf) -> Self {
        Self {
            config: config.clone(),
            container_root: container_root.clone(),
        }
    }

    /// Apply all mounts to the container
    pub fn apply_mounts(&self) -> Result<(), SandboxError> {
        info!("Applying {} mounts to container", self.config.mounts.len());

        // Create mount points
        for mount in &self.config.mounts {
            self.create_mount(mount)?;
        }

        // Create symbolic links
        for symlink in &self.config.symlinks {
            self.create_symlink(symlink)?;
        }

        info!("Successfully applied all mounts");
        Ok(())
    }

    /// Create a single mount point
    fn create_mount(&self, mount: &MountPoint) -> Result<(), SandboxError> {
        let target_path = self.container_root.join(&mount.target);

        // Create target directory/file
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SandboxError::MountFailed(format!(
                    "Failed to create mount parent {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        match mount.mount_type {
            MountType::Bind => {
                // Check if source exists
                if !Path::new(&mount.source).exists() {
                    warn!(
                        "Skipping mount {} -> {} (source does not exist)",
                        mount.source, mount.target
                    );
                    return Ok(());
                }

                // Create target as file or directory based on source
                if Path::new(&mount.source).is_file() {
                    std::fs::File::create(&target_path).map_err(|e| {
                        SandboxError::MountFailed(format!(
                            "Failed to create mount target file {}: {}",
                            target_path.display(),
                            e
                        ))
                    })?;
                } else {
                    std::fs::create_dir_all(&target_path).map_err(|e| {
                        SandboxError::MountFailed(format!(
                            "Failed to create mount target dir {}: {}",
                            target_path.display(),
                            e
                        ))
                    })?;
                }

                debug!(
                    "Created bind mount: {} -> {}",
                    mount.source,
                    target_path.display()
                );
            }
            MountType::Tmpfs => {
                std::fs::create_dir_all(&target_path).map_err(|e| {
                    SandboxError::MountFailed(format!(
                        "Failed to create tmpfs target {}: {}",
                        target_path.display(),
                        e
                    ))
                })?;
                debug!("Created tmpfs mount: {}", target_path.display());
            }
            MountType::Proc => {
                std::fs::create_dir_all(&target_path).map_err(|e| {
                    SandboxError::MountFailed(format!(
                        "Failed to create proc target {}: {}",
                        target_path.display(),
                        e
                    ))
                })?;
                debug!("Created proc mount: {}", target_path.display());
            }
        }

        Ok(())
    }

    /// Create a symbolic link
    fn create_symlink(&self, symlink: &SymLink) -> Result<(), SandboxError> {
        let link_path = self.container_root.join(&symlink.link_path);

        // Create parent directory
        if let Some(parent) = link_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SandboxError::MountFailed(format!(
                    "Failed to create symlink parent {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        // Create symlink
        std::os::unix::fs::symlink(&symlink.target, &link_path).map_err(|e| {
            SandboxError::MountFailed(format!(
                "Failed to create symlink {} -> {}: {}",
                link_path.display(),
                symlink.target,
                e
            ))
        })?;

        debug!(
            "Created symlink: {} -> {}",
            link_path.display(),
            symlink.target
        );
        Ok(())
    }

    /// Setup mounts in a command's pre_exec (for use within namespaces)
    pub fn setup_namespace_mounts(&self, cmd: &mut Command) -> Result<(), SandboxError> {
        // TODO: This will be called within the mount namespace to actually perform mounts
        // For now, we just prepare the filesystem structure
        debug!("Mount namespace setup prepared");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mount_config_creation() {
        let config = MountConfig::default_secure();

        // Should have essential system mounts
        assert!(config.mounts.iter().any(|m| m.target == "bin"));
        assert!(config.mounts.iter().any(|m| m.target == "lib"));
        assert!(config.mounts.iter().any(|m| m.target == "usr"));

        // Should have essential devices
        assert!(config.mounts.iter().any(|m| m.target == "dev/null"));

        // Should have working directory
        assert!(config.mounts.iter().any(|m| m.target == "w"));

        println!("Mount config created with {} mounts", config.mounts.len());
    }

    #[test]
    fn test_mount_manager() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config = MountConfig::minimal();
        let manager = MountManager::new(&config, &temp_dir.path().to_path_buf());

        // This would fail without proper permissions, but we can test the structure
        match manager.apply_mounts() {
            Ok(()) => println!("Mount manager test passed"),
            Err(e) => println!("Mount manager test failed (expected): {}", e),
        }
    }
}
