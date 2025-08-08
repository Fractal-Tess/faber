//! Container filesystem mounting operations.
//!
//! This module handles all aspects of setting up and tearing down the container's
//! filesystem isolation. It manages various types of mounts including:
//!
//! - **Folder mounts**: Bind mounting host directories into the container
//! - **Tmpfs mounts**: Creating temporary filesystems for ephemeral storage
//! - **Device mounts**: Providing controlled access to specific device files
//! - **File mounts**: Bind mounting individual configuration files
//! - **Essential filesystems**: Setting up /proc and /sys for system functionality
//!
//! The mounting process follows a specific security model:
//! 1. Initial bind mount to establish the filesystem connection
//! 2. Set mount propagation to private to prevent namespace leakage
//! 3. Apply security flags (nosuid, nodev, readonly) through remounting
//!
//! All mount operations are designed to provide strong isolation while maintaining
//! the functionality needed for code execution within the container.

use std::fs;
use std::path::Path;

use nix::mount::{MntFlags, MsFlags, mount, umount2};
use nix::sys::stat::{self as statx, Mode, SFlag, mknod};
use std::os::unix::fs::PermissionsExt;
use tracing::{debug, warn};

use super::errors::ContainerError;
use crate::{
    config::{
        FileMount, FilePermissions, FolderMount, FolderPermissions, MountsConfig, TempfsMount,
    },
    container::ContainerRuntime,
};

impl ContainerRuntime {
    /// Sets up all container mounts in the correct order for proper isolation.
    ///
    /// This function orchestrates the complete mount setup process for a container.
    /// The order of operations is critical:
    /// 1. **Folders** - Host directories that may contain submounts
    /// 2. **Tmpfs** - Temporary filesystems for work/tmp directories
    /// 3. **Devices** - Device files for controlled system access
    /// 4. **Files** - Individual configuration files
    /// 5. **Essential** - System filesystems (/proc, /sys)
    ///
    /// Each mount type follows a security-first approach with isolation and
    /// restriction of privileges where possible.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Configuration specifying what to mount and how
    ///
    /// # Returns
    /// * `Ok(())` - All mounts were set up successfully
    /// * `Err(ContainerError)` - Any mount operation failed
    ///
    /// # Security Considerations
    /// - All mounts use private propagation to prevent namespace leakage
    /// - Security flags (nosuid, nodev) are applied to prevent privilege escalation
    /// - Read-only remounts are applied based on configuration
    pub fn setup_mounts(root: &Path, mounts: &MountsConfig) -> Result<(), ContainerError> {
        // Ensure base directories exist for tmpfs mounts declared as work/tmp
        // for rel in [
        //     fs_cfg.work_dir.target.as_str(),
        //     fs_cfg.tmp_dir.target.as_str(),
        // ] {
        //     let dir = root.join(rel);
        //     fs::create_dir_all(&dir).map_err(|e| ContainerError::CreateDir {
        //         path: dir.clone(),
        //         source: e,
        //     })?;
        // }

        // Mount necessary folders in the container (/bin, /lib, ...etc)
        Self::mount_folders(root, &mounts.folders)?;

        // Mount the tmpfs mounts in the container (work/tmp)
        Self::mount_tmpfs(root, &mounts.tmpfs)?;

        // Mount the devices in the container (/dev/random,)
        Self::mount_devices(root)?;

        // Mount the files in the container (config files, ...etc)
        Self::mount_files(root, &mounts.files)?;

        // Mount the essential filesystems (/proc and /sys)
        Self::mount_proc_and_sys(root)?;

        Ok(())
    }
    /// - Hardware detection tools need /sys for device information
    fn mount_proc_and_sys(root: &Path) -> Result<(), ContainerError> {
        // /proc
        let proc_dir = root.join("proc");
        fs::create_dir_all(&proc_dir).map_err(|e| ContainerError::CreateDir {
            path: proc_dir.clone(),
            source: e,
        })?;
        mount(
            Some("proc"),
            &proc_dir,
            Some("proc"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            Option::<&str>::None,
        )
        .map_err(|e| ContainerError::MountProc {
            tgt: proc_dir.clone(),
            source: e,
        })?;

        // /sys (read-only)
        let sys_dir = root.join("sys");
        fs::create_dir_all(&sys_dir).map_err(|e| ContainerError::CreateDir {
            path: sys_dir.clone(),
            source: e,
        })?;
        mount(
            Some("sysfs"),
            &sys_dir,
            Some("sysfs"),
            MsFlags::MS_RDONLY | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            Option::<&str>::None,
        )
        .map_err(|e| ContainerError::MountSys {
            tgt: sys_dir.clone(),
            source: e,
        })?;

        Ok(())
    }

    /// Unmounts all container filesystems in reverse order for safe cleanup.
    ///
    /// This function performs a complete teardown of the container's filesystem
    /// mounts. The unmounting happens in reverse order of mounting to handle
    /// dependencies correctly:
    /// 1. **Files** - Individual file mounts
    /// 2. **Devices** - Device file mounts  
    /// 3. **Tmpfs** - Temporary filesystems
    /// 4. **Folders** - Host directory mounts
    ///
    /// The function uses lazy unmounting (MNT_DETACH) to handle cases where
    /// processes might still have open file descriptors. It continues attempting
    /// to unmount all filesystems even if some fail, logging warnings for failures.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Configuration specifying what was mounted
    ///
    /// # Returns
    /// * `Ok(())` - All unmounts succeeded or no mounts were present
    /// * `Err(ContainerError)` - At least one unmount operation failed (last error)
    ///
    /// # Error Handling
    /// - Continues unmounting even after failures to clean up as much as possible
    /// - Logs warnings for individual unmount failures
    /// - Returns the last error encountered, if any
    pub fn umount_all(root: &Path, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        if let Err(e) = Self::umount_files(root, mounts) {
            warn!("Unmount files failed at {}: {}", root.display(), e);
            last_err = Some(e);
        }
        if let Err(e) = Self::umount_devices(root, mounts) {
            warn!("Unmount devices failed at {}: {}", root.display(), e);
            last_err = Some(e);
        }
        if let Err(e) = Self::umount_tmpfs(root, mounts) {
            warn!("Unmount tmpfs failed at {}: {}", root.display(), e);
            last_err = Some(e);
        }
        if let Err(e) = Self::umount_folders(root, mounts) {
            warn!("Unmount folders failed at {}: {}", root.display(), e);
            last_err = Some(e);
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn mount_folders(root: &Path, folders: &Vec<FolderMount>) -> Result<(), ContainerError> {
        for m in folders {
            // The target path of the mount
            let target = root.join(&m.target);

            let flags = MsFlags::MS_BIND
                | MsFlags::MS_NODEV
                | MsFlags::MS_NOSUID
                | MsFlags::MS_REC
                | MsFlags::MS_PRIVATE
                | if let FolderPermissions::ReadOnly = m.permissions {
                    MsFlags::MS_RDONLY
                } else {
                    MsFlags::empty()
                };

            fs::create_dir_all(&target).map_err(|e| ContainerError::CreateDir {
                path: target.clone(),
                source: e,
            })?;

            mount(
                Some(Path::new(&m.source)),
                &target,
                Option::<&str>::None,
                flags,
                Option::<&str>::None,
            )
            .map_err(|e| ContainerError::MountFolder {
                name: m.name.clone(),
                src: m.source.clone(),
                tgt: target.clone(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Creates tmpfs (temporary filesystem) mounts for ephemeral storage.
    ///
    /// This function creates in-memory filesystems that provide fast, temporary
    /// storage for the container. Tmpfs mounts are commonly used for:
    /// - Work directories where code execution happens
    /// - Temporary file storage that should not persist
    /// - High-performance storage for compilation artifacts
    ///
    /// The tmpfs mounts are created with security restrictions but allow execution,
    /// making them suitable for work directories where programs need to run.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory  
    /// * `mounts` - Mount configuration containing tmpfs specifications
    ///
    /// # Returns
    /// * `Ok(())` - All tmpfs mounts created successfully
    /// * `Err(ContainerError)` - Any mount operation failed
    ///
    /// # Mount Options
    /// The function accepts mount options (like size limits) that are passed
    /// directly to the tmpfs filesystem. Common options include:
    /// - `size=100M` - Limit tmpfs to 100MB
    /// - `nr_inodes=1024` - Limit number of inodes
    /// - `mode=0755` - Set directory permissions
    ///
    /// # Security Features
    /// - **MS_NOSUID**: Prevents privilege escalation via setuid programs
    /// - **MS_NODEV**: Prevents device access (tmpfs doesn't contain devices anyway)
    /// - **Execution allowed**: Unlike other mounts, tmpfs allows program execution
    fn mount_tmpfs(root: &Path, tmpfs: &Vec<TempfsMount>) -> Result<(), ContainerError> {
        for m in tmpfs {
            let target = root.join(&m.target);

            fs::create_dir_all(&target).map_err(|e| ContainerError::CreateDir {
                path: target.clone(),
                source: e,
            })?;

            let flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV; // allow execution in work/tmp

            mount(
                Option::<&str>::None,
                &target,
                Some("tmpfs"),
                flags,
                Some(m.options.as_str()),
            )
            .map_err(|e| ContainerError::MountTmpfs {
                name: m.name.clone(),
                tgt: target.clone(),
                options: m.options.clone(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Mounts device files into the container with controlled access.
    ///
    /// This function provides controlled access to specific device files within
    /// the container. Unlike folder mounts, device mounts preserve device semantics
    /// by not applying the MS_NODEV flag initially, allowing proper device access.
    ///
    /// The function follows a three-step process:
    /// 1. **Create placeholder** - Ensures the target device file exists
    /// 2. **Bind mount** - Links the host device to the container location
    /// 3. **Security remount** - Applies restrictions while preserving device access
    ///
    /// Common device mounts include:
    /// - `/dev/null` - For discarding output
    /// - `/dev/zero` - For reading null bytes
    /// - `/dev/urandom` - For random number generation
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Mount configuration containing device specifications
    ///
    /// # Returns
    /// * `Ok(())` - All device mounts completed successfully
    /// * `Err(ContainerError)` - Any mount operation failed
    ///
    /// # Security Considerations
    /// - Device semantics are preserved (no MS_NODEV on initial mount)
    /// - MS_NOSUID prevents privilege escalation
    /// - Optional read-only remounting for additional security
    /// - Private propagation prevents mount event leakage
    fn mount_devices(root: &Path) -> Result<(), ContainerError> {
        // Fixed set of essential devices to create inside the container
        let device_specs: &[(&str, &str, bool)] = &[
            ("dev_null", "/dev/null", true),
            ("dev_zero", "/dev/zero", true),
            ("dev_random", "/dev/random", true),
            ("dev_urandom", "/dev/urandom", true),
            ("dev_full", "/dev/full", true),
        ];

        for (_name, target_rel, is_rw) in device_specs.iter().copied() {
            let target = root.join(target_rel.trim_start_matches('/'));

            // Ensure parent directory exists
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| ContainerError::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            // Discover source device metadata (type and rdev)
            let src_path = Path::new(target_rel);
            let src_stat = statx::stat(src_path).map_err(|e| ContainerError::StatPath {
                path: src_path.to_path_buf(),
                source: e,
            })?;
            let src_kind_bits = SFlag::from_bits_truncate(src_stat.st_mode);
            let dev_kind = if src_kind_bits.contains(SFlag::S_IFBLK) {
                SFlag::S_IFBLK
            } else {
                SFlag::S_IFCHR
            };
            let dev_rdev = src_stat.st_rdev;

            // If target exists but is not the same device node, remove it
            if target.exists() {
                if let Ok(tgt_stat) = statx::stat(&target) {
                    let tgt_kind_bits = SFlag::from_bits_truncate(tgt_stat.st_mode);
                    let is_dev = tgt_kind_bits.contains(SFlag::S_IFCHR)
                        || tgt_kind_bits.contains(SFlag::S_IFBLK);
                    if !is_dev || tgt_stat.st_rdev != dev_rdev || !tgt_kind_bits.contains(dev_kind)
                    {
                        fs::remove_file(&target).map_err(|e| ContainerError::RemovePath {
                            path: target.clone(),
                            source: e,
                        })?;
                    }
                } else {
                    // Could not stat target, attempt to remove to recreate
                    if let Err(e) = fs::remove_file(&target) {
                        return Err(ContainerError::RemovePath {
                            path: target.clone(),
                            source: e,
                        });
                    }
                }
            }

            // Create the device node with appropriate permissions
            let mode_bits: u32 = if is_rw { 0o666 } else { 0o444 };
            let mode = Mode::from_bits_truncate(mode_bits);
            mknod(&target, dev_kind, mode, dev_rdev).map_err(|e| ContainerError::CreateDevice {
                path: target.clone(),
                source: e,
            })?;

            // Ensure permissions are set as intended (in case node existed)
            let perms = fs::Permissions::from_mode(mode_bits);
            fs::set_permissions(&target, perms).map_err(|e| ContainerError::SetPermissions {
                path: target.clone(),
                octal_mode: mode_bits,
                source: e,
            })?;
        }
        Ok(())
    }

    /// Mounts individual files into the container filesystem.
    ///
    /// This function performs bind mounts of individual files from the host
    /// into the container. This is commonly used for configuration files,
    /// certificates, or other single files that need to be available inside
    /// the container.
    ///
    /// The process involves:
    /// 1. **Create placeholder** - Ensures the target file exists for binding
    /// 2. **Bind mount** - Links the host file to the container location
    /// 3. **Private propagation** - Isolates mount events
    /// 4. **Security remount** - Applies nosuid, nodev, and optional readonly
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Mount configuration containing file specifications
    ///
    /// # Returns
    /// * `Ok(())` - All file mounts completed successfully
    /// * `Err(ContainerError)` - Any mount operation failed
    ///
    /// # Use Cases
    /// - Configuration files that should be read-only
    /// - SSL certificates for secure communications
    /// - License files required by software
    /// - Shared libraries needed by applications
    ///
    /// # Security Features
    /// - **MS_NOSUID**: Prevents setuid execution from mounted files
    /// - **MS_NODEV**: Prevents device interpretation (files aren't devices)
    /// - **MS_RDONLY**: Applied based on file permission configuration
    /// - **MS_PRIVATE**: Prevents mount propagation to host namespace
    fn mount_files(root: &Path, files: &Vec<FileMount>) -> Result<(), ContainerError> {
        for m in files {
            let target = root.join(&m.target);
            debug!(
                "Mounting file '{}' from {} to {}",
                m.name,
                m.source,
                target.display()
            );
            // Ensure parent directory exists, then create placeholder file
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| ContainerError::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            fs::File::create(&target).map_err(|e| ContainerError::CreateFile {
                path: target.clone(),
                source: e,
            })?;

            // Initial bind
            let initial_flags = MsFlags::MS_BIND;
            mount(
                Some(Path::new(&m.source)),
                &target,
                Option::<&str>::None,
                initial_flags,
                Option::<&str>::None,
            )
            .map_err(|e| ContainerError::MountFile {
                name: m.name.clone(),
                src: m.source.clone(),
                tgt: target.clone(),
                source: e,
            })?;

            // Set mount propagation to private
            mount(
                Option::<&str>::None,
                &target,
                Option::<&str>::None,
                MsFlags::MS_PRIVATE,
                Option::<&str>::None,
            )
            .map_err(|e| ContainerError::SetPrivate {
                tgt: target.clone(),
                source: e,
            })?;

            // Remount with security flags
            let mut remount_flags =
                MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_NOSUID | MsFlags::MS_NODEV;
            if let FilePermissions::ReadOnly = m.permissions {
                remount_flags |= MsFlags::MS_RDONLY;
            }
            mount(
                Option::<&str>::None,
                &target,
                Option::<&str>::None,
                remount_flags,
                Option::<&str>::None,
            )
            .map_err(|e| ContainerError::RemountFile {
                name: m.name.clone(),
                tgt: target.clone(),
                source: e,
            })?;
        }
        Ok(())
    }

    /// Mounts essential system filesystems (/proc and /sys) for container functionality.
    ///
    /// This function sets up the fundamental system filesystems that many programs
    /// expect to be available. These virtual filesystems provide access to:
    ///
    /// - **/proc**: Process and system information
    ///   - Process lists and status
    ///   - System memory information  
    ///   - Kernel parameters and statistics
    ///   - Required by many system utilities
    ///
    /// - **/sys**: System and hardware information  
    ///   - Device information and attributes
    ///   - Kernel module information
    ///   - Hardware topology
    ///   - Mounted read-only for security
    ///
    /// Both filesystems are mounted with strict security flags to prevent
    /// privilege escalation and unauthorized system access.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    ///
    /// # Returns
    /// * `Ok(())` - Both essential filesystems mounted successfully
    /// * `Err(ContainerError)` - Either /proc or /sys mount failed
    ///
    /// # Security Flags Applied
    /// - **MS_NOSUID**: No setuid program execution
    /// - **MS_NODEV**: No device file access
    /// - **MS_NOEXEC**: No program execution from these filesystems
    /// - **MS_RDONLY**: /sys is mounted read-only for additional security
    ///
    /// # Why These Are Essential
    /// Many programs and utilities expect these filesystems to be present:
    /// - `ps` command needs /proc for process information
    /// - Memory management tools need /proc/meminfo
    ///
    /// Unmounts folder bind mounts in reverse order for safe cleanup.
    ///
    /// This function unmounts all folder bind mounts that were created during
    /// container setup. It processes the mounts in reverse order to handle
    /// any dependencies between mounts correctly.
    ///
    /// The function uses lazy unmounting (MNT_DETACH) which immediately
    /// disconnects the filesystem from the namespace but delays the actual
    /// unmount until all references are released.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Mount configuration containing folder specifications
    ///
    /// # Returns
    /// * `Ok(())` - All folder unmounts succeeded
    /// * `Err(ContainerError)` - At least one unmount failed (returns last error)
    ///
    /// # Error Handling
    /// - Continues processing all mounts even if some fail
    /// - Logs warnings for individual failures
    /// - Returns the last error encountered
    fn umount_folders(root: &Path, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        for m in mounts.folders.iter().rev() {
            let target = root.join(&m.target);
            debug!("Unmounting folder '{}' from {}", m.name, target.display());
            if let Err(e) = umount2(&target, MntFlags::MNT_DETACH) {
                warn!("Failed to unmount {}: {}", target.display(), e);
                last_err = Some(ContainerError::Unmount {
                    tgt: target.clone(),
                    source: e,
                });
            }
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Unmounts tmpfs filesystems in reverse order for safe cleanup.
    ///
    /// This function unmounts all tmpfs (temporary filesystem) mounts that
    /// were created during container setup. Since tmpfs filesystems are
    /// entirely in memory, unmounting them immediately frees the associated
    /// memory resources.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Mount configuration containing tmpfs specifications
    ///
    /// # Returns
    /// * `Ok(())` - All tmpfs unmounts succeeded
    /// * `Err(ContainerError)` - At least one unmount failed (returns last error)
    ///
    /// # Memory Management
    /// - Tmpfs unmounting immediately releases memory back to the system
    /// - No data persistence concerns since tmpfs is ephemeral
    /// - Uses lazy unmounting for robustness against open file descriptors
    fn umount_tmpfs(root: &Path, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        for m in mounts.tmpfs.iter().rev() {
            let target = root.join(&m.target);
            debug!("Unmounting tmpfs '{}' from {}", m.name, target.display());
            if let Err(e) = umount2(&target, MntFlags::MNT_DETACH) {
                warn!("Failed to unmount {}: {}", target.display(), e);
                last_err = Some(ContainerError::Unmount {
                    tgt: target.clone(),
                    source: e,
                });
            }
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }

    /// Unmounts device file bind mounts in reverse order for safe cleanup.
    ///
    /// This function unmounts all device file bind mounts that were created
    /// during container setup. Device mounts typically include essential
    /// devices like /dev/null, /dev/zero, and /dev/urandom.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Mount configuration containing device specifications
    ///
    /// # Returns
    /// * `Ok(())` - All device unmounts succeeded
    /// * `Err(ContainerError)` - At least one unmount failed (returns last error)
    ///
    /// # Device Access
    /// - Unmounting device files immediately removes container access
    /// - Host device files remain unaffected
    /// - Uses lazy unmounting for robustness
    fn umount_devices(_root: &Path, _mounts: &MountsConfig) -> Result<(), ContainerError> {
        // No-op: device nodes are created with mknod and are not separate mounts
        Ok(())
    }

    /// Unmounts individual file bind mounts in reverse order for safe cleanup.
    ///
    /// This function unmounts all individual file bind mounts that were created
    /// during container setup. File mounts typically include configuration files,
    /// certificates, and other single files needed by the container.
    ///
    /// # Arguments
    /// * `root` - Path to the container's root directory
    /// * `mounts` - Mount configuration containing file specifications
    ///
    /// # Returns
    /// * `Ok(())` - All file unmounts succeeded
    /// * `Err(ContainerError)` - At least one unmount failed (returns last error)
    ///
    /// # File Safety
    /// - Unmounting files immediately removes container access
    /// - Host files remain unmodified and accessible
    /// - Uses lazy unmounting to handle any remaining open file descriptors
    fn umount_files(root: &Path, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        for m in mounts.files.iter().rev() {
            let target = root.join(&m.target);
            debug!("Unmounting file '{}' from {}", m.name, target.display());
            if let Err(e) = umount2(&target, MntFlags::MNT_DETACH) {
                warn!("Failed to unmount {}: {}", target.display(), e);
                last_err = Some(ContainerError::Unmount {
                    tgt: target.clone(),
                    source: e,
                });
            }
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }
}
