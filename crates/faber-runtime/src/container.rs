use std::env::set_current_dir;
use std::fs::{create_dir_all, remove_dir, remove_dir_all};
use std::path::{Path, PathBuf};

use nix::mount::{MntFlags, MsFlags, mount, umount2};
use nix::sched::CloneFlags;
use nix::sched::unshare;
use nix::sys::stat::{Mode, SFlag, makedev, mknod};
use nix::unistd::sethostname;

use crate::prelude::*;
use crate::utils::generate_random_string;

pub struct Container {
    id: String,
    container_root_dir: PathBuf,
    workdir: PathBuf,
    tmpdir_size: String,
    workdir_size: String,
    ro_bind_mounts: Vec<&'static str>,
    w_bind_mounts: Vec<&'static str>,
}
impl Default for Container {
    fn default() -> Self {
        let id = generate_random_string(12);
        let container_root_dir = PathBuf::from(format!("/tmp/faber/{}", id));
        let ro_bind_mounts = vec!["/bin", "/lib", "/lib64", "/usr"];
        let w_bind_mounts = vec!["/tmp"];
        let workdir = PathBuf::from("/faber");
        let tmpdir_size = "128M".to_string();
        let workdir_size = "128M".to_string();
        Self {
            id,
            container_root_dir,
            workdir,
            tmpdir_size,
            workdir_size,
            ro_bind_mounts,
            w_bind_mounts,
        }
    }
}

impl Container {
    pub fn setup(&self) -> Result<()> {
        self.create_container_root_dir()?;

        let unshare_flags = CloneFlags::CLONE_NEWUTS // hostname
            | CloneFlags::CLONE_NEWNET // network
            | CloneFlags::CLONE_NEWIPC // ipc
            | CloneFlags::CLONE_NEWNS; // mount

        unshare(unshare_flags).map_err(|e| FaberError::Unshare { e })?;

        self.rebind_root()?;
        self.rebind_new_root()?;
        self.bind_mounts()?;
        self.pivot_root()?;
        self.create_dev_devices()?;
        self.create_proc()?;
        self.create_sys()?;
        self.create_cgroup()?;
        self.create_tmpdir()?;
        self.change_hostname()?;
        self.create_workdir()?;

        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        let _ = umount2(&self.container_root_dir, MntFlags::MNT_DETACH);

        remove_dir_all(&self.container_root_dir).map_err(|e| {
            FaberError::RemoveContainerRootDir {
                e,
                details: "Failed to remove container root directory".to_string(),
            }
        })?;
        Ok(())
    }

    pub fn mask_paths() -> Result<()> {
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
        create_dir_all(&self.container_root_dir).map_err(|e| {
            FaberError::CreateContainerRootDir {
                e,
                details: "Failed to create container root directory".to_string(),
            }
        })?;

        Ok(())
    }

    fn rebind_root(&self) -> Result<()> {
        mount(
            None::<&str>,
            "/",
            None::<&str>,
            MsFlags::MS_REC | MsFlags::MS_PRIVATE,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to remount root as private".to_string(),
        })?;

        Ok(())
    }

    fn bind_mounts(&self) -> Result<()> {
        for source in &self.ro_bind_mounts {
            // Check if source exists before mounting
            if !std::path::Path::new(source).exists() {
                println!("⚠️  Skipping mount for non-existent path: {}", source);
                continue;
            }

            let target = self
                .container_root_dir
                .join(source.strip_prefix("/").unwrap_or(source));

            // Create target directory and its parent
            create_dir_all(&target).map_err(|e| FaberError::CreateDir {
                e,
                details: "Failed to create target directory".to_string(),
            })?;

            // Use MS_BIND without MS_RDONLY initially, then remount as read-only
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
            self.container_root_dir
                .to_str()
                .ok_or(FaberError::CreateContainerRootDir {
                    e: std::io::Error::other(
                        "Failed to convert container root directory to string",
                    ),
                    details: "Failed to convert container root directory to string".to_string(),
                })?;

        // First, bind mount the new root to itself
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
        let new_root = self.container_root_dir.to_path_buf();
        let old_root = self.container_root_dir.join("oldroot");

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
        mknod("/dev/null", flags, mode, device_id).map_err(|source| FaberError::MkDevDevice {
            detaills: "Failed to create null device".to_string(),
        })?;

        let device_id = makedev(1, 5);
        mknod("/dev/zero", flags, mode, device_id).map_err(|source| FaberError::MkDevDevice {
            detaills: "Failed to create zero device".to_string(),
        })?;

        let device_id = makedev(1, 7);
        mknod("/dev/full", flags, mode, device_id).map_err(|source| FaberError::MkDevDevice {
            detaills: "Failed to create full device".to_string(),
        })?;

        let device_id = makedev(1, 8);
        mknod("/dev/random", flags, mode, device_id).map_err(|source| FaberError::MkDevDevice {
            detaills: "Failed to create random device".to_string(),
        })?;

        let device_id = makedev(1, 9);
        mknod("/dev/urandom", flags, mode, device_id).map_err(|source| {
            FaberError::MkDevDevice {
                detaills: "Failed to create urandom device".to_string(),
            }
        })?;

        Ok(())
    }

    fn create_workdir(&self) -> Result<()> {
        create_dir_all(&self.workdir).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create workdir".to_string(),
        })?;

        // Mount tmpfs with specified size and mode 0777 (readable, writable, executable by everyone)
        let mount_options = format!("size={},mode=0777", self.workdir_size);
        let workdir_str = self.workdir.to_str().ok_or_else(|| FaberError::Generic {
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

        set_current_dir(&self.workdir).map_err(|e| FaberError::Chdir {
            e,
            details: "Failed to change current directory to workdir".to_string(),
        })?;

        Ok(())
    }

    fn create_proc(&self) -> Result<()> {
        let proc_path = "/proc";
        let proc_fstype = "proc";
        let proc_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        create_dir_all(proc_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create proc directory".to_string(),
        })?;

        mount(
            None::<&str>,
            proc_path,
            Some(proc_fstype),
            proc_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount proc filesystem".to_string(),
        })?;

        Ok(())
    }

    fn create_sys(&self) -> Result<()> {
        let sys_target = "/sys";
        let sys_fstype = "sysfs";
        let sys_flags = MsFlags::MS_NODEV | MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC;

        create_dir_all(sys_target).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create sys directory".to_string(),
        })?;

        mount(
            None::<&str>,
            sys_target,
            Some(sys_fstype),
            sys_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount sys filesystem".to_string(),
        })?;

        Ok(())
    }

    fn create_cgroup(&self) -> Result<()> {
        let cgroup_path = "/sys/fs/cgroup";
        let cgroup_fstype = "cgroup2";
        let cgroup_flags =
            MsFlags::MS_RELATIME | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC;

        mount(
            None::<&str>,
            cgroup_path,
            Some(cgroup_fstype),
            cgroup_flags,
            None::<&str>,
        )
        .map_err(|e| FaberError::Mount {
            e,
            details: "Failed to mount cgroup2 filesystem".to_string(),
        })?;

        Ok(())
    }

    fn create_tmpdir(&self) -> Result<()> {
        let target = PathBuf::from("/tmp");
        create_dir_all(&target).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create tmp directory".to_string(),
        })?;

        let mount_options = format!("size={},mode=1777", self.tmpdir_size);
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
        sethostname("faber").map_err(|e| FaberError::SetHostname {
            e,
            details: "Failed to change hostname".to_string(),
        })?;

        Ok(())
    }
}
