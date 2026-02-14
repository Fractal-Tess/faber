use std::{
    env::set_current_dir,
    fs::{create_dir_all, remove_dir, remove_dir_all},
    path::{Path, PathBuf},
};

use nix::{
    mount::{mount, umount2, MntFlags, MsFlags},
    sched::unshare,
    sched::CloneFlags,
    sys::stat::{makedev, mknod, Mode, SFlag},
    unistd::sethostname,
};

use crate::{container::config::ContainerConfig, prelude::*};

#[derive(Default)]
pub struct Container {
    config: ContainerConfig,
}

impl Container {
    pub(crate) fn new(config: ContainerConfig) -> Self {
        Self { config }
    }

    pub(crate) fn setup(&self) -> Result<()> {
        self.create_container_root_dir()?;

        let unshare_flags = CloneFlags::CLONE_NEWUTS
            | CloneFlags::CLONE_NEWNET
            | CloneFlags::CLONE_NEWIPC
            | CloneFlags::CLONE_NEWNS
            | CloneFlags::CLONE_NEWPID;

        unshare(unshare_flags).map_err(|e| FaberError::Unshare { e })?;

        self.rebind_new_root()?;
        self.bind_mounts()?;
        self.bind_dev_devices()?;
        self.pivot_root()?;
        self.create_proc()?;
        self.create_sys()?;
        self.create_cgroup()?;
        self.create_tmpdir()?;
        self.change_hostname()?;
        self.create_workdir()?;
        self.unmount_oldroot()?;

        Ok(())
    }

    pub(crate) fn cleanup(&self) -> Result<()> {
        // Unmount filesystems mounted in newroot before removing directory
        let _ = umount2(
            &self.config.container_root_dir.join("proc"),
            MntFlags::MNT_DETACH,
        );
        let _ = umount2(
            &self.config.container_root_dir.join("sys/fs/cgroup"),
            MntFlags::MNT_DETACH,
        );
        let _ = umount2(
            &self.config.container_root_dir.join("sys"),
            MntFlags::MNT_DETACH,
        );
        let _ = umount2(&self.config.container_root_dir, MntFlags::MNT_DETACH);

        remove_dir_all(&self.config.container_root_dir).map_err(|e| {
            FaberError::RemoveContainerRootDir {
                e,
                details: "Failed to remove container root directory".to_string(),
            }
        })?;
        Ok(())
    }

    pub(crate) fn mask_paths() -> Result<()> {
        umount2("/sys", MntFlags::MNT_DETACH).map_err(|e| FaberError::Umount {
            e,
            details: "Failed to unmount sys".to_string(),
        })?;
        umount2("/proc", MntFlags::MNT_DETACH).map_err(|e| FaberError::Umount {
            e,
            details: "Failed to unmount proc".to_string(),
        })?;

        mount(
            Some("tmpfs"),
            "/sys",
            Some("tmpfs"),
            MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
            Some("size=0"),
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount tmpfs to sys".to_string(),
        })?;

        mount(
            Some("tmpfs"),
            "/proc",
            Some("tmpfs"),
            MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC,
            Some("size=0"),
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount tmpfs to proc".to_string(),
        })?;

        Ok(())
    }

    fn create_container_root_dir(&self) -> Result<()> {
        create_dir_all(&self.config.container_root_dir).map_err(|e| {
            FaberError::CreateContainerRootDir {
                e,
                details: "Failed to create container root directory".to_string(),
            }
        })?;

        Ok(())
    }

    fn bind_mounts(&self) -> Result<()> {
        for source in &self.config.bind_mounts_ro {
            if !Path::new(source).exists() {
                println!("⚠️  Skipping mount for non-existent path: {}", source);
                continue;
            }

            let target = self
                .config
                .container_root_dir
                .join(source.strip_prefix("/").unwrap_or(source));

            create_dir_all(&target).map_err(|e| FaberError::CreateDir {
                e,
                details: "Failed to create target directory".to_string(),
            })?;

            mount(
                Some(*source),
                target.as_os_str(),
                None::<&str>,
                MsFlags::MS_BIND | MsFlags::MS_RDONLY,
                None::<&str>,
            )
            .map_err(|e| FaberError::Mount {
                e,
                details: format!("Failed to bind mount {} to {:?}", source, target),
            })?;
        }
        Ok(())
    }

    fn rebind_new_root(&self) -> Result<()> {
        let target =
            self.config
                .container_root_dir
                .to_str()
                .ok_or(FaberError::CreateContainerRootDir {
                    e: std::io::Error::other(
                        "Failed to convert container root directory to string",
                    ),
                    details: "Failed to convert container root directory to string".to_string(),
                })?;

        mount(
            Some(target),
            target,
            None::<&str>,
            MsFlags::MS_BIND | MsFlags::MS_REC,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to bind mount new root".to_string(),
        })?;

        Ok(())
    }

    fn pivot_root(&self) -> Result<()> {
        let new_root = self.config.container_root_dir.to_path_buf();
        let old_root = self.config.container_root_dir.join("oldroot");

        create_dir_all(&old_root).map_err(|source| FaberError::CreateDir {
            e: source,
            details: "Failed to create old root directory".to_string(),
        })?;

        let new_root_str = new_root.to_str().ok_or_else(|| FaberError::Generic {
            message: "Failed to convert new root directory to string".to_string(),
        })?;

        let old_root_str = old_root.to_str().ok_or_else(|| FaberError::Generic {
            message: "Failed to convert old root directory to string".to_string(),
        })?;

        nix::unistd::pivot_root(new_root_str, old_root_str).map_err(|e| FaberError::PivotRoot {
            e,
            details: "Failed to pivot root".to_string(),
        })?;

        set_current_dir("/").map_err(|e| FaberError::Chdir {
            e,
            details: "Failed to change current directory".to_string(),
        })?;

        // Note: oldroot is NOT unmounted here - mounts need to be moved first
        // The cleanup happens in move_mounts_to_newroot() and final cleanup

        Ok(())
    }

    fn unmount_oldroot(&self) -> Result<()> {
        umount2("/oldroot", MntFlags::MNT_DETACH).map_err(|e| FaberError::Umount {
            e,
            details: "Failed to unmount old root".to_string(),
        })?;

        remove_dir("/oldroot").map_err(|e| FaberError::RemoveDir {
            e,
            details: "Failed to remove old root directory".to_string(),
        })?;

        Ok(())
    }

    fn bind_dev_devices(&self) -> Result<()> {
        let dev_dir = self.config.container_root_dir.join("dev");
        create_dir_all(&dev_dir).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create dev directory in container root".to_string(),
        })?;

        let devices = [
            "/dev/null",
            "/dev/zero",
            "/dev/full",
            "/dev/random",
            "/dev/urandom",
        ];

        for device in &devices {
            let target = self.config.container_root_dir.join(&device[1..]);

            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&target)
                .map_err(|e| FaberError::MkDevDevice {
                    detaills: format!("Failed to create mount point for {}", device),
                    e: nix::errno::Errno::from_raw(e.raw_os_error().unwrap_or(0)),
                })?;

            mount(
                Some(*device),
                target.as_os_str(),
                None::<&str>,
                MsFlags::MS_BIND,
                None::<&str>,
            )
            .map_err(|e| FaberError::Mount {
                e,
                details: format!("Failed to bind mount {}", device),
            })?;
        }

        Ok(())
    }

    fn create_dev_devices(&self) -> Result<()> {
        let flags = SFlag::S_IFCHR;
        let mode = Mode::S_IRUSR
            | Mode::S_IWUSR
            | Mode::S_IRGRP
            | Mode::S_IWGRP
            | Mode::S_IROTH
            | Mode::S_IWOTH;

        create_dir_all("/dev").map_err(|e| FaberError::CreateDir {
            e,
            details: ("Failed to create dev directory".to_string()),
        })?;

        let device_id = makedev(1, 3);
        mknod("/dev/null", flags, mode, device_id).map_err(|e| FaberError::MkDevDevice {
            detaills: "Failed to create null device".to_string(),
            e,
        })?;

        let device_id = makedev(1, 5);
        mknod("/dev/zero", flags, mode, device_id).map_err(|e| FaberError::MkDevDevice {
            detaills: "Failed to create zero device".to_string(),
            e,
        })?;

        let device_id = makedev(1, 7);
        mknod("/dev/full", flags, mode, device_id).map_err(|e| FaberError::MkDevDevice {
            detaills: "Failed to create full device".to_string(),
            e,
        })?;

        let device_id = makedev(1, 8);
        mknod("/dev/random", flags, mode, device_id).map_err(|e| FaberError::MkDevDevice {
            detaills: "Failed to create random device".to_string(),
            e,
        })?;

        let device_id = makedev(1, 9);
        mknod("/dev/urandom", flags, mode, device_id).map_err(|e| FaberError::MkDevDevice {
            detaills: "Failed to create urandom device".to_string(),
            e,
        })?;

        Ok(())
    }

    fn create_workdir(&self) -> Result<()> {
        create_dir_all(&self.config.workdir).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create workdir".to_string(),
        })?;

        let mount_options = format!("size={},mode=0777", self.config.workdir_size);
        let workdir_str = self
            .config
            .workdir
            .to_str()
            .ok_or_else(|| FaberError::Generic {
                message: "Failed to convert workdir to string".to_string(),
            })?;

        mount(
            Some("tmpfs"),
            workdir_str,
            Some("tmpfs"),
            MsFlags::empty(),
            Some(mount_options.as_str()),
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: format!("Failed to mount tmpfs workdir to {}", workdir_str),
        })?;

        set_current_dir(&self.config.workdir).map_err(|e| FaberError::Chdir {
            e,
            details: "Failed to change current directory to workdir".to_string(),
        })?;

        Ok(())
    }

    fn create_proc(&self) -> Result<()> {
        let proc_path = "/proc";

        create_dir_all(proc_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create proc directory".to_string(),
        })?;

        match mount(
            Some("/oldroot/proc"),
            proc_path,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        ) {
            Ok(_) => {
                tracing::debug!("Bind mounted /oldroot/proc to /proc");
            }
            Err(e) => {
                tracing::debug!("Bind mount failed, attempting fresh proc mount: {:?}", e);
                let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;
                mount(
                    None::<&str>,
                    proc_path,
                    Some("proc"),
                    proc_flags,
                    None::<&str>,
                )
                .map_err(|e| FaberError::Mount {
                    e,
                    details: "Failed to mount proc filesystem".to_string(),
                })?;
            }
        }

        Ok(())
    }

    fn create_sys(&self) -> Result<()> {
        let sys_target = "/sys";

        create_dir_all(sys_target).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create sys directory".to_string(),
        })?;

        // Try to mount new sysfs first
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;
        if mount(
            None::<&str>,
            sys_target,
            Some("sysfs"),
            sys_flags,
            None::<&str>,
        )
        .is_ok()
        {
            return Ok(());
        }

        // Fallback: bind mount from oldroot/sys
        mount(
            Some("/oldroot/sys"),
            sys_target,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to bind mount sys from oldroot".to_string(),
        })?;

        Ok(())
    }

    fn create_cgroup(&self) -> Result<()> {
        let cgroup_path = "/sys/fs/cgroup";

        create_dir_all(cgroup_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create cgroup directory".to_string(),
        })?;

        // Try to mount new cgroup2 filesystem first
        let cgroup_flags =
            MsFlags::MS_RELATIME | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;
        if mount(
            None::<&str>,
            cgroup_path,
            Some("cgroup2"),
            cgroup_flags,
            None::<&str>,
        )
        .is_ok()
        {
            return Ok(());
        }

        // Fallback: bind mount from oldroot/sys/fs/cgroup
        mount(
            Some("/oldroot/sys/fs/cgroup"),
            cgroup_path,
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to bind mount cgroup from oldroot".to_string(),
        })?;

        Ok(())
    }

    fn create_proc_in_newroot(&self) -> Result<()> {
        let proc_path = self.config.container_root_dir.join("proc");
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        create_dir_all(&proc_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create proc directory in newroot".to_string(),
        })?;

        mount(
            None::<&str>,
            proc_path.as_os_str(),
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount proc filesystem in newroot".to_string(),
        })?;

        Ok(())
    }

    fn create_sys_in_newroot(&self) -> Result<()> {
        let sys_path = self.config.container_root_dir.join("sys");
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        create_dir_all(&sys_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create sys directory in newroot".to_string(),
        })?;

        mount(
            None::<&str>,
            sys_path.as_os_str(),
            Some(sys_fstype),
            sys_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount sys filesystem in newroot".to_string(),
        })?;

        Ok(())
    }

    fn create_cgroup_in_newroot(&self) -> Result<()> {
        let cgroup_path = self.config.container_root_dir.join("sys/fs/cgroup");
        let cgroup_fstype = "cgroup2";
        let cgroup_flags =
            MsFlags::MS_RELATIME | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;

        create_dir_all(&cgroup_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create cgroup directory in newroot".to_string(),
        })?;

        mount(
            None::<&str>,
            cgroup_path.as_os_str(),
            Some(cgroup_fstype),
            cgroup_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount cgroup2 filesystem in newroot".to_string(),
        })?;

        Ok(())
    }

    fn move_mounts_to_newroot(&self) -> Result<()> {
        // After pivot_root, mounts under oldroot need to be moved to new root
        // Use MS_MOVE to relocate them without unmounting
        let move_flags = MsFlags::MS_MOVE;

        // Move proc mount
        mount(
            Some("/oldroot/proc"),
            "/proc",
            None::<&str>,
            move_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to move proc mount to new root".to_string(),
        })?;

        // Move sys mount
        mount(
            Some("/oldroot/sys"),
            "/sys",
            None::<&str>,
            move_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to move sys mount to new root".to_string(),
        })?;

        Ok(())
    }

    fn create_tmpdir(&self) -> Result<()> {
        let target = PathBuf::from("/tmp");
        create_dir_all(&target).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create tmp directory".to_string(),
        })?;

        let mount_options = format!("size={},mode=1777", self.config.tmpdir_size);
        let target_str = target.to_str().ok_or_else(|| FaberError::Generic {
            message: "Failed to convert tmp directory to string".to_string(),
        })?;

        mount(
            Some("tmpfs"),
            target_str,
            Some("tmpfs"),
            MsFlags::empty(),
            Some(mount_options.as_str()),
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: format!("Failed to mount tmp filesystem to {}", target_str),
        })?;

        Ok(())
    }

    fn change_hostname(&self) -> Result<()> {
        sethostname(self.config.hostname.as_str()).map_err(|e| FaberError::SetHostname {
            e,
            details: "Failed to change hostname".to_string(),
        })?;

        Ok(())
    }
}
