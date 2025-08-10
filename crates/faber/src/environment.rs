use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::{CloneFlags, unshare},
    sys::stat::{Mode, SFlag, makedev, mknod},
    unistd::{pivot_root, sethostname},
};

use crate::{prelude::*, types::Mount};

use std::collections::HashMap;
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
        std::fs::create_dir_all(&self.container_root)?;

        self.unshare_internal()?;

        self.set_hostname_internal()?;

        self.create_proc_sys_internal()?;

        self.create_tmp_internal()?;

        self.create_work_dir_internal()?;

        self.create_devices_internal()?;

        self.bind_mounts_internal()?;

        self.pivot_root_internal()?;

        Ok(())
    }

    pub(crate) fn cleanup(&self) -> Result<()> {
        // Parent-side cleanup of the container root
        match std::fs::remove_dir_all(&self.container_root) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::GenericError(format!(
                "failed to remove container root: {e}"
            ))),
        }
    }

    pub(crate) fn write_files_to_workdir(&self, files: &HashMap<String, String>) -> Result<()> {
        let base = PathBuf::from(self.work_dir.trim_start_matches('/'));
        std::fs::create_dir_all(&base).map_err(|e| {
            Error::GenericError(format!(
                "failed to create base workdir {}: {e}",
                base.display()
            ))
        })?;
        for (rel_path, contents) in files {
            let target = base.join(rel_path);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::GenericError(format!(
                        "failed to create parent dir {} for {}: {e}",
                        parent.display(),
                        target.display()
                    ))
                })?;
            }
            std::fs::write(&target, contents).map_err(|e| {
                Error::GenericError(format!(
                    "failed to write file {} ({} bytes): {e}",
                    target.display(),
                    contents.len()
                ))
            })?;
        }
        Ok(())
    }

    fn unshare_internal(&self) -> Result<()> {
        let flags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWCGROUP;
        unshare(flags).map_err(|_| Error::UnshareFailed)?;
        Ok(())
    }

    fn set_hostname_internal(&self) -> Result<()> {
        sethostname(self.hostname.as_str()).map_err(Error::NixError)?;
        Ok(())
    }

    fn bind_mounts_internal(&self) -> Result<()> {
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        for m in &self.mounts {
            if !Path::new(&m.source).exists() {
                continue;
            }
            let target = format!(
                "{}/{}",
                self.container_root.display(),
                m.target.strip_prefix("/").unwrap().to_owned()
            );
            let flags = m
                .flags
                .iter()
                .fold(MsFlags::empty(), |acc, flag| acc | *flag);

            std::fs::create_dir_all(&target)?;

            mount(
                Some(m.source.as_str()),
                target.as_str(),
                None::<&str>,
                flags,
                m.data.as_deref(),
            )?
        }
        Ok(())
    }

    fn create_proc_sys_internal(&self) -> Result<()> {
        let proc_source = Some("proc");
        let proc_path = format!("{}/proc", self.container_root.display());
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        std::fs::create_dir_all(&proc_path)?;

        mount(
            proc_source,
            proc_path.as_str(),
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        let sys_source = Some("sysfs");
        let sys_target = format!("{}/sys", self.container_root.display());
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        std::fs::create_dir_all(&sys_target)?;

        mount(
            sys_source,
            sys_target.as_str(),
            Some(sys_fstype),
            sys_flags,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        Ok(())
    }

    fn create_tmp_internal(&self) -> Result<()> {
        let tmp_path = format!("{}/tmp", self.container_root.display());
        std::fs::create_dir_all(&tmp_path)?;
        mount(
            Some("tmpfs"),
            tmp_path.as_str(),
            Some("tmpfs"),
            MsFlags::empty(),
            Some("size=128M,mode=1777"),
        )
        .map_err(Error::NixError)?;
        Ok(())
    }

    fn create_work_dir_internal(&self) -> Result<()> {
        let work_dir = format!("{}/{}", self.container_root.display(), self.work_dir);
        std::fs::create_dir_all(&work_dir)?;
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

        let dev_path = format!("{}/dev", self.container_root.display());
        std::fs::create_dir_all(&dev_path)?;

        let device_path = format!("{dev_path}/null");
        let device_id = makedev(1, 3);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        let device_path = format!("{dev_path}/zero");
        let device_id = makedev(1, 5);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        let device_path = format!("{dev_path}/full");
        let device_id = makedev(1, 7);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        let device_path = format!("{dev_path}/random");
        let device_id = makedev(1, 8);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        let device_path = format!("{dev_path}/urandom");
        let device_id = makedev(1, 9);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        Ok(())
    }

    fn pivot_root_internal(&self) -> Result<()> {
        let new_root = self.container_root.clone();
        let old_root = format!("{}/oldroot", self.container_root.display());

        std::fs::create_dir_all(&new_root)
            .map_err(|e| Error::GenericError(format!("failed to create new_root dir: {e}")))?;
        std::fs::create_dir_all(&old_root)
            .map_err(|e| Error::GenericError(format!("failed to create old_root dir: {e}")))?;

        mount(
            Some(new_root.to_str().unwrap()),
            new_root.to_str().unwrap(),
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        pivot_root(new_root.to_str().unwrap(), old_root.as_str()).map_err(Error::NixError)?;

        std::env::set_current_dir("/")
            .map_err(|e| Error::GenericError(format!("chdir to new root failed: {e}")))?;

        umount2("/oldroot", MntFlags::MNT_DETACH).map_err(Error::NixError)?;
        let _ = std::fs::remove_dir_all("/oldroot");

        Ok(())
    }
}
