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

        self.unshare_internal()?;
        self.set_hostname_internal()?;
        // self.create_proc_sys_internal()?;
        self.create_tmp_internal()?;
        self.create_work_dir_internal()?;
        self.create_devices_internal()?;
        self.bind_mounts_internal()?;
        self.pivot_root_internal()?;
        Ok(())
    }

    pub(crate) fn cleanup(&self) -> Result<()> {
        remove_dir_all(&self.container_root).map_err(|source| Error::RemoveDir {
            path: self.container_root.clone(),
            source,
        })
    }

    pub(crate) fn write_files_to_workdir(&self, files: &HashMap<String, String>) -> Result<()> {
        let base = PathBuf::from(self.work_dir.trim_start_matches('/'));
        create_dir_all(&base).map_err(|source| Error::CreateDir {
            path: base.clone(),
            source,
        })?;
        for (rel_path, contents) in files {
            let target = base.join(rel_path);
            if let Some(parent) = target.parent() {
                create_dir_all(parent).map_err(|source| Error::CreateDir {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            write(&target, contents).map_err(|source| Error::WriteFile {
                path: target.clone(),
                bytes: contents.len(),
                source,
            })?;
        }
        Ok(())
    }

    fn unshare_internal(&self) -> Result<()> {
        // Note: PID namespace (CLONE_NEWPID) has special behavior
        // unshare(CLONE_NEWPID) only affects child processes, not the calling process
        // The calling process itself doesn't enter the new PID namespace
        // Only child processes created AFTER the unshare will be in the new namespace
        //
        // This means that when we call this method, we're still in the host's PID namespace
        // Child processes created later (like when we fork in the runtime) will inherit
        // the new PID namespace
        let flags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWCGROUP;

        unshare(flags).map_err(|source| Error::Unshare { flags, source })?;
        Ok(())
    }

    fn set_hostname_internal(&self) -> Result<()> {
        sethostname(self.hostname.as_str()).map_err(|source| Error::SetHostname {
            hostname: self.hostname.clone(),
            source,
        })?;
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
        .map_err(|err| Error::Mount {
            src: "(none)".into(),
            target: "/".into(),
            fstype: None,
            flags: MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            err,
        })?;

        for m in &self.mounts {
            if !Path::new(&m.source).exists() {
                continue;
            }
            let target = format!(
                "{}/{target}",
                self.container_root.display(),
                target = m.target.strip_prefix("/").unwrap().to_owned()
            );
            let flags = m
                .flags
                .iter()
                .fold(MsFlags::empty(), |acc, flag| acc | *flag);

            create_dir_all(&target).map_err(|source| Error::CreateDir {
                path: PathBuf::from(&target),
                source,
            })?;

            mount(
                Some(m.source.as_str()),
                target.as_str(),
                None::<&str>,
                flags,
                m.data.as_deref(),
            )
            .map_err(|err| Error::Mount {
                src: m.source.clone(),
                target: target.clone(),
                fstype: None,
                flags,
                err,
            })?
        }
        Ok(())
    }

    fn create_proc_sys_internal(&self) -> Result<()> {
        let proc_source = Some("proc");
        let proc_path = format!("{}/proc", self.container_root.display());
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        create_dir_all(&proc_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&proc_path),
            source,
        })?;

        mount(
            proc_source,
            proc_path.as_str(),
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(|err| Error::Mount {
            src: "proc".into(),
            target: proc_path.clone(),
            fstype: Some(proc_fstype.into()),
            flags: proc_flags,
            err,
        })?;

        let sys_source = Some("sysfs");
        let sys_target = format!("{}/sys", self.container_root.display());
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        create_dir_all(&sys_target).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&sys_target),
            source,
        })?;

        mount(
            sys_source,
            sys_target.as_str(),
            Some(sys_fstype),
            sys_flags,
            None::<&str>,
        )
        .map_err(|err| Error::Mount {
            src: "sysfs".into(),
            target: sys_target.clone(),
            fstype: Some(sys_fstype.into()),
            flags: sys_flags,
            err,
        })?;

        Ok(())
    }

    fn create_tmp_internal(&self) -> Result<()> {
        let tmp_path = format!("{}/tmp", self.container_root.display());
        create_dir_all(&tmp_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&tmp_path),
            source,
        })?;
        mount(
            Some("tmpfs"),
            tmp_path.as_str(),
            Some("tmpfs"),
            MsFlags::empty(),
            Some("size=128M,mode=1777"),
        )
        .map_err(|err| Error::Mount {
            src: "tmpfs".into(),
            target: tmp_path.clone(),
            fstype: Some("tmpfs".into()),
            flags: MsFlags::empty(),
            err,
        })?;
        Ok(())
    }

    fn create_work_dir_internal(&self) -> Result<()> {
        let work_dir = format!("{}/{}", self.container_root.display(), self.work_dir);
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

        let dev_path = format!("{}/dev", self.container_root.display());
        create_dir_all(&dev_path).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&dev_path),
            source,
        })?;

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

        create_dir_all(&new_root).map_err(|source| Error::CreateDir {
            path: new_root.clone(),
            source,
        })?;
        create_dir_all(&old_root).map_err(|source| Error::CreateDir {
            path: PathBuf::from(&old_root),
            source,
        })?;

        mount(
            Some(new_root.to_str().unwrap()),
            new_root.to_str().unwrap(),
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(|err| Error::Mount {
            src: new_root.display().to_string(),
            target: new_root.display().to_string(),
            fstype: None,
            flags: MsFlags::MS_BIND | MsFlags::MS_REC,
            err,
        })?;

        pivot_root(new_root.to_str().unwrap(), old_root.as_str()).map_err(|source| {
            Error::PivotRoot {
                new_root: new_root.clone(),
                old_root: PathBuf::from(&old_root),
                source,
            }
        })?;

        set_current_dir("/").map_err(|source| Error::Chdir {
            path: "/".into(),
            source,
        })?;

        umount2("/oldroot", MntFlags::MNT_DETACH).map_err(|err| Error::Umount {
            target: "/oldroot".into(),
            flags: MntFlags::MNT_DETACH,
            err,
        })?;
        let _ = remove_dir_all("/oldroot");

        Ok(())
    }
}
