use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::{CloneFlags, unshare},
    sys::stat::{Mode, SFlag, makedev, mknod},
    unistd::{pivot_root, sethostname},
};

use crate::{prelude::*, types::Mount};

use std::collections::HashMap;
use std::env::set_current_dir;
use std::fs::{create_dir_all, remove_dir_all, write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct ContainerEnvironment {
    pub(crate) container_root: PathBuf,
    pub(crate) hostname: String,
    pub(crate) mounts: Vec<Mount>,
    pub(crate) work_dir: String,
}

impl ContainerEnvironment {
    pub(crate) fn new(
        container_root: PathBuf,
        hostname: String,
        mounts: Vec<Mount>,
        work_dir: String,
    ) -> Self {
        Self {
            container_root,
            hostname,
            mounts,
            work_dir,
        }
    }

    pub(crate) fn initialize(&self) -> Result<()> {
        create_dir_all(&self.container_root).map_err(|source| Error::CreateDir {
            path: self.container_root.clone(),
            source,
        })?;

        self.unshare_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "unshare namespaces".to_string(),
                details: format!("Failed to unshare namespaces: {e}"),
            })?;

        self.bind_mounts_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "bind mounts".to_string(),
                details: format!("Failed to setup bind mounts: {e}"),
            })?;

        self.pivot_root_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "pivot root".to_string(),
                details: format!("Failed to pivot root: {e}"),
            })?;

        self.create_devices_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "create devices".to_string(),
                details: format!("Failed to create device nodes: {e}"),
            })?;

        self.set_hostname_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "set hostname".to_string(),
                details: format!("Failed to set hostname: {e}"),
            })?;

        self.create_proc_sys_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "create proc/sys".to_string(),
                details: format!("Failed to create proc/sys filesystems: {e}"),
            })?;

        self.create_tmp_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "create tmp".to_string(),
                details: format!("Failed to create tmp filesystem: {e}"),
            })?;

        self.create_work_dir_internal()
            .map_err(|e| Error::ContainerEnvironment {
                operation: "create work directory".to_string(),
                details: format!("Failed to create work directory: {e}"),
            })?;

        Ok(())
    }

    pub(crate) fn cleanup(&self) -> Result<()> {
        // Remove container root
        remove_dir_all(&self.container_root).map_err(|source| Error::RemoveDir {
            path: self.container_root.clone(),
            source,
        })?;
        Ok(())
    }

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

    fn set_hostname_internal(&self) -> Result<()> {
        // Set hostname
        sethostname(self.hostname.as_str()).map_err(|source| Error::SetHostname {
            hostname: self.hostname.clone(),
            source,
        })?;
        Ok(())
    }

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
        for (_i, m) in self.mounts.iter().enumerate() {
            // Skip if source does not exist
            if !Path::new(&m.source).exists() {
                continue;
            }

            // Target within container
            let target = format!(
                "{}/{target}",
                self.container_root.display(),
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

    fn create_proc_sys_internal(&self) -> Result<()> {
        // Proc
        let proc_source = Some("proc");
        let proc_path = "/proc";
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        // Create proc dir
        create_dir_all(proc_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(proc_path),
            source,
        })?;

        // Mount proc
        mount(
            proc_source,
            proc_path,
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: "proc".to_string(),
            target: proc_path.to_string(),
            fstype: Some(proc_fstype.to_string()),
            flags: proc_flags,
            err: e,
        })?;

        // Sys
        let sys_source = Some("sysfs");
        let sys_target = "/sys";
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        // Create sys dir
        create_dir_all(sys_target).map_err(|source| Error::CreateDir {
            path: PathBuf::from(sys_target),
            source,
        })?;

        // Mount sys
        mount(
            sys_source,
            sys_target,
            Some(sys_fstype),
            sys_flags,
            None::<&str>,
        )
        .map_err(|e| Error::Mount {
            src: "sysfs".to_string(),
            target: sys_target.to_string(),
            fstype: Some(sys_fstype.to_string()),
            flags: sys_flags,
            err: e,
        })?;

        Ok(())
    }

    fn create_tmp_internal(&self) -> Result<()> {
        let tmp_path = "/tmp";

        // Create tmp dir
        create_dir_all(tmp_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(tmp_path),
            source,
        })?;

        // Mount tmp
        mount(
            Some("tmpfs"),
            tmp_path,
            Some("tmpfs"),
            MsFlags::empty(),
            Some("size=128M,mode=1777"),
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

    fn create_work_dir_internal(&self) -> Result<()> {
        let work_dir = format!("/{}", self.work_dir.trim_start_matches('/'));
        create_dir_all(&work_dir).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&work_dir),
            source,
        })?;
        Ok(())
    }

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

    fn pivot_root_internal(&self) -> Result<()> {
        // New root path (which is essentially the container root)
        let new_root = self.container_root.clone();

        // Old root path (which is /oldroot -> host root `/`)
        let old_root = format!("{}/oldroot", self.container_root.display());

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
            flags: MntFlags::MNT_DETACH,
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
