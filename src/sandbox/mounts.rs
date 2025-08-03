//! Mount configuration for container sandboxes
//!
//! This module handles filesystem mount operations and configurations
//! needed for container isolation.

use crate::sandbox::{Result, SandboxError};
use nix::mount::{MsFlags, mount, umount};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Mount types supported by the sandbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MountType {
    /// Bind mount from host
    Bind,
    /// Temporary filesystem (tmpfs)
    Tmpfs,
    /// Proc filesystem
    Proc,
    /// Sysfs filesystem  
    Sysfs,
    /// Devtmpfs for device files
    Devtmpfs,
}

/// Mount configuration for a single mount point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    /// Source path (for bind mounts) or filesystem type
    pub source: String,
    /// Target path inside container
    pub target: PathBuf,
    /// Mount type
    pub mount_type: MountType,
    /// Mount flags
    pub flags: Vec<String>,
    /// Mount options
    pub options: Vec<String>,
    /// Whether mount is read-only
    pub read_only: bool,
}

impl Mount {
    /// Create a new bind mount
    pub fn bind<P: AsRef<Path>>(source: P, target: P, read_only: bool) -> Self {
        Self {
            source: source.as_ref().to_string_lossy().to_string(),
            target: target.as_ref().to_path_buf(),
            mount_type: MountType::Bind,
            flags: vec![],
            options: vec![],
            read_only,
        }
    }

    /// Create a new tmpfs mount
    pub fn tmpfs<P: AsRef<Path>>(target: P, size_mb: u64) -> Self {
        Self {
            source: "tmpfs".to_string(),
            target: target.as_ref().to_path_buf(),
            mount_type: MountType::Tmpfs,
            flags: vec![],
            options: vec![format!("size={}m", size_mb)],
            read_only: false,
        }
    }

    /// Create a new proc mount
    pub fn proc<P: AsRef<Path>>(target: P) -> Self {
        Self {
            source: "proc".to_string(),
            target: target.as_ref().to_path_buf(),
            mount_type: MountType::Proc,
            flags: vec![],
            options: vec![],
            read_only: false,
        }
    }

    /// Create a new sysfs mount
    pub fn sysfs<P: AsRef<Path>>(target: P) -> Self {
        Self {
            source: "sysfs".to_string(),
            target: target.as_ref().to_path_buf(),
            mount_type: MountType::Sysfs,
            flags: vec![],
            options: vec![],
            read_only: true,
        }
    }

    /// Create a new devtmpfs mount
    pub fn devtmpfs<P: AsRef<Path>>(target: P) -> Self {
        Self {
            source: "udev".to_string(),
            target: target.as_ref().to_path_buf(),
            mount_type: MountType::Devtmpfs,
            flags: vec![],
            options: vec![],
            read_only: false,
        }
    }

    /// Perform the mount operation
    pub fn mount(&self, root_path: &Path) -> Result<()> {
        let target_path = root_path.join(&self.target);

        // Create target directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if !target_path.exists() {
            if target_path.extension().is_some() {
                // Create file for file mounts
                std::fs::File::create(&target_path)?;
            } else {
                // Create directory for directory mounts
                std::fs::create_dir_all(&target_path)?;
            }
        }

        let mut flags = MsFlags::empty();

        // Apply read-only flag
        if self.read_only {
            flags |= MsFlags::MS_RDONLY;
        }

        // Apply bind mount flag for bind mounts
        if matches!(self.mount_type, MountType::Bind) {
            flags |= MsFlags::MS_BIND;
        }

        // Add nodev, nosuid, noexec for security
        flags |= MsFlags::MS_NODEV | MsFlags::MS_NOSUID;

        // Don't add noexec for certain mounts that need execution
        match self.mount_type {
            MountType::Bind | MountType::Tmpfs => {
                // These may need execution permissions
            }
            _ => {
                flags |= MsFlags::MS_NOEXEC;
            }
        }

        debug!(
            "Mounting {} -> {} (type: {:?}, flags: {:?})",
            self.source,
            target_path.display(),
            self.mount_type,
            flags
        );

        let options_joined = if self.options.is_empty() {
            String::new()
        } else {
            self.options.join(",")
        };

        let options_str = if options_joined.is_empty() {
            None
        } else {
            Some(options_joined.as_str())
        };

        mount(
            Some(self.source.as_str()),
            &target_path,
            match self.mount_type {
                MountType::Bind => None,
                MountType::Tmpfs => Some("tmpfs"),
                MountType::Proc => Some("proc"),
                MountType::Sysfs => Some("sysfs"),
                MountType::Devtmpfs => Some("devtmpfs"),
            },
            flags,
            options_str,
        )
        .map_err(|e| {
            SandboxError::MountFailed(format!("Failed to mount {}: {}", self.target.display(), e))
        })?;

        info!(
            "Successfully mounted {} -> {}",
            self.source,
            target_path.display()
        );
        Ok(())
    }

    /// Unmount this mount point
    pub fn unmount(&self, root_path: &Path) -> Result<()> {
        let target_path = root_path.join(&self.target);

        debug!("Unmounting {}", target_path.display());

        umount(&target_path).map_err(|e| {
            SandboxError::MountFailed(format!(
                "Failed to unmount {}: {}",
                target_path.display(),
                e
            ))
        })?;

        info!("Successfully unmounted {}", target_path.display());
        Ok(())
    }
}

/// Configuration for all mounts in a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    /// List of mounts to create
    pub mounts: Vec<Mount>,
    /// Root filesystem path
    pub root_path: PathBuf,
}

impl MountConfig {
    /// Create a new mount configuration
    pub fn new<P: AsRef<Path>>(root_path: P) -> Self {
        Self {
            mounts: Vec::new(),
            root_path: root_path.as_ref().to_path_buf(),
        }
    }

    /// Add a mount to the configuration
    pub fn add_mount(mut self, mount: Mount) -> Self {
        self.mounts.push(mount);
        self
    }

    /// Create default mounts for a basic container
    pub fn default_mounts<P: AsRef<Path>>(root_path: P) -> Self {
        let mut config = Self::new(root_path);

        // Add essential filesystem mounts
        config = config
            .add_mount(Mount::proc("/proc"))
            .add_mount(Mount::sysfs("/sys"))
            .add_mount(Mount::devtmpfs("/dev"))
            .add_mount(Mount::tmpfs("/tmp", 100)) // 100MB tmp
            .add_mount(Mount::tmpfs("/var/tmp", 50)); // 50MB var/tmp

        config
    }

    /// Apply all mounts in the configuration
    pub fn apply_mounts(&self) -> Result<()> {
        info!(
            "Applying {} mounts to {}",
            self.mounts.len(),
            self.root_path.display()
        );

        for mount in &self.mounts {
            mount.mount(&self.root_path)?;
        }

        info!("Successfully applied all mounts");
        Ok(())
    }

    /// Remove all mounts (in reverse order)
    pub fn cleanup_mounts(&self) -> Result<()> {
        info!(
            "Cleaning up {} mounts from {}",
            self.mounts.len(),
            self.root_path.display()
        );

        // Unmount in reverse order
        for mount in self.mounts.iter().rev() {
            if let Err(e) = mount.unmount(&self.root_path) {
                tracing::warn!("Failed to unmount {}: {}", mount.target.display(), e);
                // Continue with other unmounts even if one fails
            }
        }

        info!("Mount cleanup completed");
        Ok(())
    }
}
