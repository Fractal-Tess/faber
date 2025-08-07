use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use nix::mount::{MntFlags, MsFlags, mount, umount2};
use tracing::{debug, warn};

use crate::config::{
    ContainerFilesystemConfig, DevicePermissions, FilePermissions, FolderPermissions, MountsConfig,
};

#[derive(thiserror::Error, Debug)]
pub enum ContainerError {
    #[error("Create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Set permissions 0{octal_mode:o} on {path}: {source}")]
    SetPermissions {
        path: PathBuf,
        octal_mode: u32,
        #[source]
        source: std::io::Error,
    },

    #[error("Create file {path}: {source}")]
    CreateFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Mount folder '{name}' {src} -> {tgt}: {source}")]
    MountFolder {
        name: String,
        src: String,
        tgt: PathBuf,
        #[source]
        source: nix::Error,
    },

    #[error("Remount folder '{name}' at {tgt} read-only: {source}")]
    RemountFolder {
        name: String,
        tgt: PathBuf,
        #[source]
        source: nix::Error,
    },

    #[error("Mount tmpfs '{name}' at {tgt} with opts '{options}': {source}")]
    MountTmpfs {
        name: String,
        tgt: PathBuf,
        options: String,
        #[source]
        source: nix::Error,
    },

    #[error("Mount device '{name}' {src} -> {tgt}: {source}")]
    MountDevice {
        name: String,
        src: String,
        tgt: PathBuf,
        #[source]
        source: nix::Error,
    },

    #[error("Remount device '{name}' at {tgt} read-only: {source}")]
    RemountDevice {
        name: String,
        tgt: PathBuf,
        #[source]
        source: nix::Error,
    },

    #[error("Mount file '{name}' {src} -> {tgt}: {source}")]
    MountFile {
        name: String,
        src: String,
        tgt: PathBuf,
        #[source]
        source: nix::Error,
    },

    #[error("Remount file '{name}' at {tgt} read-only: {source}")]
    RemountFile {
        name: String,
        tgt: PathBuf,
        #[source]
        source: nix::Error,
    },

    #[error("Unmount {tgt}: {source}")]
    Unmount {
        tgt: PathBuf,
        #[source]
        source: nix::Error,
    },

    #[error("Remove directory {path}: {source}")]
    RemoveDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ContainerRuntime {
    request_id: String,
    root: PathBuf,
    fs_cfg: ContainerFilesystemConfig,
}

impl ContainerRuntime {
    /// Create a new container runtime instance. This does not touch the filesystem.
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

    /// Prepare the container root and mounts.
    pub fn prepare(&self) -> Result<(), ContainerError> {
        self.create_container_root()?;
        self.setup_mounts()?;
        Ok(())
    }

    /// Cleanup mounts and remove the container root directory.
    pub fn cleanup(&self) -> Result<(), ContainerError> {
        self.umount_all()?;
        fs::remove_dir_all(&self.root).map_err(|e| ContainerError::RemoveDir {
            path: self.root.clone(),
            source: e,
        })?;
        Ok(())
    }

    /// Expose the container root path
    pub fn root(&self) -> &Path {
        &self.root
    }

    // Internal helpers
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
        // TODO: Why Permissions?
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

        self.mount_folders(&self.fs_cfg.mounts)?;
        self.mount_tmpfs(&self.fs_cfg.mounts)?;
        self.mount_devices(&self.fs_cfg.mounts)?;
        self.mount_files(&self.fs_cfg.mounts)?;

        Ok(())
    }

    fn umount_all(&self) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        if let Err(e) = self.umount_files(&self.fs_cfg.mounts) {
            warn!("Unmount files failed at {}: {}", self.root.display(), e);
            last_err = Some(e);
        }
        if let Err(e) = self.umount_devices(&self.fs_cfg.mounts) {
            warn!("Unmount devices failed at {}: {}", self.root.display(), e);
            last_err = Some(e);
        }
        if let Err(e) = self.umount_tmpfs(&self.fs_cfg.mounts) {
            warn!("Unmount tmpfs failed at {}: {}", self.root.display(), e);
            last_err = Some(e);
        }
        if let Err(e) = self.umount_folders(&self.fs_cfg.mounts) {
            warn!("Unmount folders failed at {}: {}", self.root.display(), e);
            last_err = Some(e);
        }
        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(())
        }
    }

    fn mount_folders(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        for m in &mounts.folders {
            let target = self.root.join(&m.target);
            debug!(
                "Mounting folder '{}' from {} to {}",
                m.name,
                m.source,
                target.display()
            );
            fs::create_dir_all(&target).map_err(|e| ContainerError::CreateDir {
                path: target.clone(),
                source: e,
            })?;
            let flags = MsFlags::MS_BIND
                | MsFlags::MS_REC
                | MsFlags::MS_NOSUID
                | MsFlags::MS_NODEV
                | MsFlags::MS_NOEXEC;
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
            if let FolderPermissions::ReadOnly = m.permissions {
                let remount_flags = flags | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY;
                mount(
                    Some(Path::new(&m.source)),
                    &target,
                    Option::<&str>::None,
                    remount_flags,
                    Option::<&str>::None,
                )
                .map_err(|e| ContainerError::RemountFolder {
                    name: m.name.clone(),
                    tgt: target.clone(),
                    source: e,
                })?;
            }
        }
        Ok(())
    }

    fn mount_tmpfs(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        for m in &mounts.tmpfs {
            let target = self.root.join(&m.target);
            debug!(
                "Mounting tmpfs '{}' at {} with options '{}'",
                m.name,
                target.display(),
                m.options
            );
            fs::create_dir_all(&target).map_err(|e| ContainerError::CreateDir {
                path: target.clone(),
                source: e,
            })?;
            let flags = MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;
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

    fn mount_devices(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        for m in &mounts.devices {
            let target = self.root.join(&m.target);
            debug!(
                "Mounting device '{}' from {} to {}",
                m.name,
                m.source,
                target.display()
            );
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| ContainerError::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            if !target.exists() {
                fs::File::create(&target).map_err(|e| ContainerError::CreateFile {
                    path: target.clone(),
                    source: e,
                })?;
            }
            let mut flags =
                MsFlags::MS_BIND | MsFlags::MS_REC | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;
            mount(
                Some(Path::new(&m.source)),
                &target,
                Option::<&str>::None,
                flags,
                Option::<&str>::None,
            )
            .map_err(|e| ContainerError::MountDevice {
                name: m.name.clone(),
                src: m.source.clone(),
                tgt: target.clone(),
                source: e,
            })?;
            if let DevicePermissions::ReadOnly = m.permissions {
                flags |= MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY;
                mount(
                    Some(Path::new(&m.source)),
                    &target,
                    Option::<&str>::None,
                    flags,
                    Option::<&str>::None,
                )
                .map_err(|e| ContainerError::RemountDevice {
                    name: m.name.clone(),
                    tgt: target.clone(),
                    source: e,
                })?;
            }
        }
        Ok(())
    }

    fn mount_files(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        for m in &mounts.files {
            let target = self.root.join(&m.target);
            debug!(
                "Mounting file '{}' from {} to {}",
                m.name,
                m.source,
                target.display()
            );
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| ContainerError::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            if !target.exists() {
                fs::File::create(&target).map_err(|e| ContainerError::CreateFile {
                    path: target.clone(),
                    source: e,
                })?;
            }
            let flags = MsFlags::MS_BIND
                | MsFlags::MS_REC
                | MsFlags::MS_NOSUID
                | MsFlags::MS_NODEV
                | MsFlags::MS_NOEXEC;
            mount(
                Some(Path::new(&m.source)),
                &target,
                Option::<&str>::None,
                flags,
                Option::<&str>::None,
            )
            .map_err(|e| ContainerError::MountFile {
                name: m.name.clone(),
                src: m.source.clone(),
                tgt: target.clone(),
                source: e,
            })?;
            if let FilePermissions::ReadOnly = m.permissions {
                let remount_flags = flags | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY;
                mount(
                    Some(Path::new(&m.source)),
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
        }
        Ok(())
    }

    fn umount_folders(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        for m in mounts.folders.iter().rev() {
            let target = self.root.join(&m.target);
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

    fn umount_tmpfs(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        for m in mounts.tmpfs.iter().rev() {
            let target = self.root.join(&m.target);
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

    fn umount_devices(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        for m in mounts.devices.iter().rev() {
            let target = self.root.join(&m.target);
            debug!("Unmounting device '{}' from {}", m.name, target.display());
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

    fn umount_files(&self, mounts: &MountsConfig) -> Result<(), ContainerError> {
        let mut last_err: Option<ContainerError> = None;
        for m in mounts.files.iter().rev() {
            let target = self.root.join(&m.target);
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
