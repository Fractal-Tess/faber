//! Mount management for container filesystem isolation
//!
//! This module provides functionality to create secure filesystem mounts
//! within containers, allowing access to essential system resources while
//! maintaining security isolation.

use nix::mount::{MntFlags, MsFlags, mount as nix_mount, umount2};

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, error, info, warn};

use super::error::SandboxError;

/// Type of mount operation
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MountType {
    /// Bind mount - mount existing directory/file
    Bind,
    /// Tmpfs - create temporary filesystem in memory
    Tmpfs,
    /// Proc - mount /proc filesystem
    Proc,
}

/// Configuration for a single mount point  
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
            options: format!("size={size}"),
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

/// Configuration for container filesystem mounts
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MountConfig {
    /// List of mount points
    pub mounts: Vec<MountPoint>,
    /// Paths to mask (block access to)
    pub masked_paths: Vec<String>,
    /// Whether to enable proc filesystem
    pub enable_proc: bool,
    /// Whether to enable proc with read-write access
    pub proc_read_write: bool,
    /// Container hostname
    pub hostname: String,
    /// Container domain name
    pub domain_name: String,
}

impl Default for MountConfig {
    fn default() -> Self {
        Self::default_secure()
    }
}

impl MountConfig {
    /// Create a secure default mount configuration
    pub fn default_secure() -> Self {
        let mut mounts = vec![
            // Essential system directories (read-only)
            MountPoint::bind_ro("/bin", "bin"),
            MountPoint::bind_ro("/lib", "lib"),
            MountPoint::bind_ro("/lib64", "lib64"),
            MountPoint::bind_ro("/usr", "usr"),
            MountPoint::bind_ro("/etc/ld.so.cache", "etc/ld.so.cache"),
            // Essential devices
            MountPoint::bind_rw("/dev/null", "dev/null"),
            MountPoint::bind_rw("/dev/zero", "dev/zero"),
            MountPoint::bind_rw("/dev/random", "dev/random"),
            MountPoint::bind_rw("/dev/urandom", "dev/urandom"),
            MountPoint::bind_rw("/dev/full", "dev/full"),
            // Work directory (tmpfs for performance)
            MountPoint::tmpfs("work", "256m,nr_inodes=4k"),
            // Temporary directory
            MountPoint::tmpfs("tmp", "128m,nr_inodes=4k"),
        ];

        // Add proc filesystem if enabled
        if true {
            // enable_proc
            mounts.push(MountPoint::proc("proc"));
        }

        let masked_paths = vec![
            // Sensitive system paths to block
            "/sys/firmware".to_string(),
            "/sys/devices/virtual/powercap".to_string(),
            "/proc/acpi".to_string(),
            "/proc/asound".to_string(),
            "/proc/kcore".to_string(),
            "/proc/keys".to_string(),
            "/proc/latency_stats".to_string(),
            "/proc/timer_list".to_string(),
            "/proc/timer_stats".to_string(),
            "/proc/sched_debug".to_string(),
            "/proc/scsi".to_string(),
            "/usr/lib/wsl/drivers".to_string(),
            "/usr/lib/wsl/lib".to_string(),
            "/sys/kernel/debug".to_string(),
            "/sys/kernel/security".to_string(),
            "/sys/fs/cgroup".to_string(),
            "/sys/fs/fuse".to_string(),
            "/sys/fs/pstore".to_string(),
            "/sys/fs/selinux".to_string(),
            "/sys/fs/tracefs".to_string(),
            "/sys/fs/bpf".to_string(),
            "/sys/fs/efivarfs".to_string(),
            "/sys/fs/configfs".to_string(),
            "/sys/fs/autofs".to_string(),
            "/sys/fs/mqueue".to_string(),
            "/sys/fs/hugetlbfs".to_string(),
            "/sys/fs/ramfs".to_string(),
            "/sys/fs/overlayfs".to_string(),
            "/sys/fs/nsfs".to_string(),
            "/sys/fs/cgroup2".to_string(),
            "/sys/fs/cgroup/unified".to_string(),
            "/sys/fs/cgroup/systemd".to_string(),
            "/sys/fs/cgroup/cpu".to_string(),
            "/sys/fs/cgroup/memory".to_string(),
            "/sys/fs/cgroup/pids".to_string(),
            "/sys/fs/cgroup/net_cls".to_string(),
            "/sys/fs/cgroup/net_prio".to_string(),
            "/sys/fs/cgroup/blkio".to_string(),
            "/sys/fs/cgroup/devices".to_string(),
            "/sys/fs/cgroup/freezer".to_string(),
            "/sys/fs/cgroup/perf_event".to_string(),
            "/sys/fs/cgroup/hugetlb".to_string(),
            "/sys/fs/cgroup/rdma".to_string(),
        ];

        Self {
            mounts,
            masked_paths,
            enable_proc: true,
            proc_read_write: false,
            hostname: "faber-container".to_string(),
            domain_name: "local".to_string(),
        }
    }
}

/// Mount manager for container filesystem isolation
pub struct MountManager {
    /// Mount configuration
    config: MountConfig,
    /// Container root path
    container_root: PathBuf,
    /// Paths to mask (block access to)
    masked_paths: Vec<String>,
}

impl MountManager {
    /// Create a new mount manager
    pub fn new(config: &MountConfig, container_root: &PathBuf) -> Self {
        Self {
            config: config.clone(),
            container_root: container_root.clone(),
            masked_paths: config.masked_paths.clone(),
        }
    }

    /// Apply mounts with specific mode
    pub fn apply_mounts(&self) -> Result<(), SandboxError> {
        // Apply all mount points
        for mount_point in &self.config.mounts {
            if let Err(e) = self.create_mount(mount_point) {
                return Err(SandboxError::MountFailed(format!(
                    "Failed to create mount {}: {}",
                    mount_point.target, e
                )));
            }
        }

        info!(
            "Successfully applied {} mount points",
            self.config.mounts.len()
        );
        Ok(())
    }

    /// Apply path masking by creating null mounts over sensitive paths
    pub fn apply_path_masking(&self) -> Result<(), SandboxError> {
        for masked_path in &self.masked_paths {
            let target_path = self
                .container_root
                .join(masked_path.trim_start_matches('/'));

            // Create parent directories if they don't exist
            if let Some(parent) = target_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    warn!(
                        "Failed to create parent directory for masked path {}: {}",
                        masked_path, e
                    );
                    continue;
                }
            }

            // Create a null mount to mask the path
            if let Err(e) = nix::mount::mount(
                Some("none"),
                &target_path,
                Some("tmpfs"),
                MsFlags::empty(),
                Some("mode=0000"),
            ) {
                warn!("Failed to mask path {}: {}", masked_path, e);
            } else {
                debug!("Masked sensitive path: {}", masked_path);
            }
        }

        Ok(())
    }

    /// Create a single mount point with actual mount syscalls
    fn create_mount(&self, mount: &MountPoint) -> Result<(), SandboxError> {
        let target_path = self.container_root.join(&mount.target);

        // Create target directory/file
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
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
                if Path::new(&mount.source).is_dir() {
                    fs::create_dir_all(&target_path).map_err(|e| {
                        SandboxError::MountFailed(format!(
                            "Failed to create mount target dir {}: {}",
                            target_path.display(),
                            e
                        ))
                    })?;
                } else {
                    fs::File::create(&target_path).map_err(|e| {
                        SandboxError::MountFailed(format!(
                            "Failed to create mount target file {}: {}",
                            target_path.display(),
                            e
                        ))
                    })?;
                }

                // Perform actual bind mount
                let mut flags = MsFlags::MS_BIND;
                if mount.readonly {
                    flags |= MsFlags::MS_RDONLY;
                }

                match nix_mount(
                    Some(Path::new(&mount.source)),
                    &target_path,
                    None::<&str>,
                    flags,
                    None::<&str>,
                ) {
                    Ok(()) => {}
                    Err(nix::Error::EPERM) => {
                        return Err(SandboxError::MountFailed(format!(
                            "Cannot bind mount with no privileges: {} -> {}",
                            mount.source,
                            target_path.display()
                        )));
                    }
                    Err(e) => {
                        error!(
                            "❌ Bind mount failed: {} -> {}: {}",
                            mount.source,
                            target_path.display(),
                            e
                        );
                        return Err(SandboxError::MountFailed(format!(
                            "Failed to bind mount {} -> {}: {}",
                            mount.source,
                            target_path.display(),
                            e
                        )));
                    }
                }

                // If readonly, remount with readonly flag
                if mount.readonly {
                    match nix_mount(
                        None::<&Path>,
                        &target_path,
                        None::<&str>,
                        MsFlags::MS_REMOUNT | MsFlags::MS_BIND | MsFlags::MS_RDONLY,
                        None::<&str>,
                    ) {
                        Ok(()) => {}
                        Err(nix::Error::EPERM) => {
                            warn!(
                                "⚠️  Readonly remount skipped (no privileges): {} - symlink is inherently readonly",
                                target_path.display()
                            );
                            // If we're using symlink fallback, we don't need to remount
                        }
                        Err(e) => {
                            error!(
                                "❌ Readonly remount failed: {} -> {}: {}",
                                mount.source,
                                target_path.display(),
                                e
                            );
                            return Err(SandboxError::MountFailed(format!(
                                "Failed to remount {} as readonly: {}",
                                target_path.display(),
                                e
                            )));
                        }
                    }
                }
            }
            MountType::Tmpfs => {
                fs::create_dir_all(&target_path).map_err(|e| {
                    SandboxError::MountFailed(format!(
                        "Failed to create tmpfs target {}: {}",
                        target_path.display(),
                        e
                    ))
                })?;

                // Perform actual tmpfs mount
                let data = if mount.options.is_empty() {
                    Some("size=128m,nr_inodes=4k")
                } else {
                    Some(mount.options.as_str())
                };

                match nix_mount(
                    None::<&Path>,
                    &target_path,
                    Some("tmpfs"),
                    MsFlags::empty(),
                    data,
                ) {
                    Ok(()) => {}
                    Err(nix::Error::EPERM) => {
                        warn!(
                            "⚠️  Tmpfs mount skipped (no privileges): {} - using regular directory",
                            target_path.display()
                        );
                        return Err(SandboxError::MountFailed(format!(
                            "Failed to mount tmpfs at {}: {}",
                            target_path.display(),
                            nix::Error::EPERM
                        )));
                    }
                    Err(e) => {
                        error!("❌ Tmpfs mount failed: {}: {}", target_path.display(), e);
                        return Err(SandboxError::MountFailed(format!(
                            "Failed to mount tmpfs at {}: {}",
                            target_path.display(),
                            e
                        )));
                    }
                }
            }
            MountType::Proc => {
                fs::create_dir_all(&target_path).map_err(|e| {
                    SandboxError::MountFailed(format!(
                        "Failed to create proc target {}: {}",
                        target_path.display(),
                        e
                    ))
                })?;

                // Perform actual proc mount
                match nix_mount(
                    None::<&Path>,
                    &target_path,
                    Some("proc"),
                    MsFlags::empty(),
                    None::<&str>,
                ) {
                    Ok(()) => {}
                    Err(nix::Error::EPERM) => {
                        warn!(
                            "⚠️  Proc mount skipped (no privileges): {} - symlinking to host /proc",
                            target_path.display()
                        );
                        return Err(SandboxError::MountFailed(format!(
                            "Failed to mount proc at {}: {}",
                            target_path.display(),
                            nix::Error::EPERM
                        )));
                    }
                    Err(e) => {
                        error!("❌ Proc mount failed: {}: {}", target_path.display(), e);
                        return Err(SandboxError::MountFailed(format!(
                            "Failed to mount proc at {}: {}",
                            target_path.display(),
                            e
                        )));
                    }
                }
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

        Ok(())
    }

    /// Setup mounts in a command's pre_exec (for use within namespaces)
    /// This function is called AFTER entering the mount namespace
    pub fn setup_namespace_mounts(&self) -> Result<(), SandboxError> {
        // Apply all mounts - this time within the mount namespace
        for mount in &self.config.mounts {
            self.create_mount(mount)?;
        }

        Ok(())
    }

    /// Unmount all mounts from the container
    /// This should be called before removing the container directory
    pub fn unmount_all(&self) -> Result<(), SandboxError> {
        // Unmount in reverse order to handle dependencies properly
        for mount in self.config.mounts.iter().rev() {
            self.unmount_single(mount)?;
        }

        Ok(())
    }

    /// Unmount a single mount point
    fn unmount_single(&self, mount: &MountPoint) -> Result<(), SandboxError> {
        let target_path = self.container_root.join(&mount.target);

        // Skip if target doesn't exist
        if !target_path.exists() {
            return Ok(());
        }

        // For bind mounts, tmpfs, and proc - attempt to unmount
        match mount.mount_type {
            MountType::Bind | MountType::Tmpfs | MountType::Proc => {
                // Try to unmount using the umount2 system call
                match umount2(&target_path, MntFlags::MNT_DETACH) {
                    Ok(()) => {}
                    Err(nix::Error::EINVAL) => {
                        // Not a mount point or already unmounted
                        debug!(
                            "⚠️  Not a mount point (already unmounted?): {}",
                            target_path.display()
                        );
                    }
                    Err(nix::Error::ENOENT) => {
                        // Target doesn't exist
                        debug!("⚠️  Target doesn't exist: {}", target_path.display());
                    }
                    Err(nix::Error::EPERM) => {
                        // No permission - might be running unprivileged or using symlinks
                        debug!(
                            "⚠️  No permission to unmount (unprivileged?): {}",
                            target_path.display()
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to unmount {}: {} - continuing cleanup",
                            target_path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
