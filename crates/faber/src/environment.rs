use nix::{
    mount::{MntFlags, MsFlags, mount, umount2},
    sched::{CloneFlags, unshare},
    sys::stat::{Mode, SFlag, makedev, mknod},
    unistd::{pivot_root, sethostname},
};

use crate::{prelude::*, types::Mount};

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

    pub(crate) fn unshare(&self) -> Result<()> {
        let flags = CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWPID
            | CloneFlags::CLONE_NEWNET;
        unshare(flags).map_err(|_| Error::UnshareFailed)?;
        Ok(())
    }

    pub(crate) fn set_hostname(&self) -> Result<()> {
        sethostname(self.hostname.as_str()).map_err(Error::NixError)?;
        Ok(())
    }

    pub(crate) fn print_entries(&self, path: &Path) -> Result<()> {
        let stat = std::fs::read_dir(path)?;
        for entry in stat {
            let entry = entry.unwrap();
            let path = entry.path();
            eprintln!("path: {path:?}");
        }
        Ok(())
    }

    pub(crate) fn bind_mounts(&self) -> Result<()> {
        // Make mount propagation private so mounts don't propagate back to host
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        // Bind mount to the container root
        for m in &self.mounts {
            // Skip mounts whose source does not exist to avoid ENOENT
            if !Path::new(&m.source).exists() {
                eprintln!("skipping mount {}: source does not exist", m.source);
                continue;
            }
            let target = format!(
                "{}/{}",
                self.container_root.display(),
                // Safe unwrap due to strip_prefix on a leading '/'
                m.target.strip_prefix("/").unwrap().to_owned()
            );
            let flags = m
                .flags
                .iter()
                .fold(MsFlags::empty(), |acc, flag| acc | *flag);

            std::fs::create_dir_all(&target)?;

            match mount(
                Some(m.source.as_str()),
                target.as_str(),
                None::<&str>,
                flags,
                m.data.as_deref(),
            ) {
                Ok(_) => {
                    eprintln!("mounted {}: {}", m.source, target);
                }
                Err(e) => {
                    eprintln!("failed to mount {}: {e:?}", m.source);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn create_proc_sys(&self) -> Result<()> {
        // Mount procfs on /proc within the container root
        let proc_source = Some("proc");
        let proc_path = format!("{}/proc", self.container_root.display());
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        std::fs::create_dir_all(&proc_path)?;
        eprintln!("[faber:mount] mounting proc -> {}", proc_path);

        mount(
            proc_source,
            proc_path.as_str(),
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        // Mount sysfs on /sys within the container root
        let sys_source = Some("sysfs");
        let sys_target = format!("{}/sys", self.container_root.display());
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        std::fs::create_dir_all(&sys_target)?;
        eprintln!("[faber:mount] mounting sysfs -> {}", sys_target);

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

    pub(crate) fn create_tmp(&self) -> Result<()> {
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

    pub(crate) fn create_work_dir(&self) -> Result<()> {
        let work_dir = format!("{}/{}", self.container_root.display(), self.work_dir);
        std::fs::create_dir_all(&work_dir)?;
        Ok(())
    }

    pub(crate) fn create_devices(&self) -> Result<()> {
        let flags = SFlag::S_IFCHR;
        let mode = Mode::S_IRUSR
            | Mode::S_IWUSR
            | Mode::S_IRGRP
            | Mode::S_IWGRP
            | Mode::S_IROTH
            | Mode::S_IWOTH;

        let dev_path = format!("{}/dev", self.container_root.display());
        std::fs::create_dir_all(&dev_path)?;

        // /dev/null
        let device_path = format!("{dev_path}/null");
        let device_id = makedev(1, 3);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // /dev/zero
        let device_path = format!("{dev_path}/zero");
        let device_id = makedev(1, 5);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // /dev/full
        let device_path = format!("{dev_path}/full");
        let device_id = makedev(1, 7);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // /dev/random
        let device_path = format!("{dev_path}/random");
        let device_id = makedev(1, 8);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        // /dev/urandom
        let device_path = format!("{dev_path}/urandom");
        let device_id = makedev(1, 9);
        let _ = mknod(device_path.as_str(), flags, mode, device_id);

        Ok(())
    }

    pub(crate) fn pivot_root(&self) -> Result<()> {
        let new_root = self.container_root.clone();
        let old_root = format!("{}/oldroot", self.container_root.display());

        std::fs::create_dir_all(&new_root)
            .map_err(|e| Error::GenericError(format!("failed to create new_root dir: {e}")))?;
        std::fs::create_dir_all(&old_root)
            .map_err(|e| Error::GenericError(format!("failed to create old_root dir: {e}")))?;

        eprintln!(
            "[faber:root] remount new_root {} as MS_BIND|MS_REC",
            new_root.display()
        );
        mount(
            Some(new_root.to_str().unwrap()),
            new_root.to_str().unwrap(),
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(Error::NixError)?;

        eprintln!(
            "[faber:root] pivot_root new_root={} old_root={}",
            new_root.display(),
            old_root
        );
        pivot_root(new_root.to_str().unwrap(), old_root.as_str()).map_err(Error::NixError)?;

        std::env::set_current_dir("/")
            .map_err(|e| Error::GenericError(format!("chdir to new root failed: {e}")))?;

        eprintln!("[faber:root] umount oldroot");
        umount2("/oldroot", MntFlags::MNT_DETACH).map_err(Error::NixError)?;
        let _ = std::fs::remove_dir_all("/oldroot");

        Ok(())
    }
}
