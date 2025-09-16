use std::fs::{create_dir_all, remove_dir, remove_dir_all};
use std::path::{Path, PathBuf};

use nix::mount::{MsFlags, mount, umount};
use nix::sched::CloneFlags;
use nix::sched::unshare;

use crate::prelude::*;
use crate::utils::generate_random_string;

pub struct Container {
    id: String,
    container_root_dir: PathBuf,
    ro_bind_mounts: Vec<&'static str>,
}
impl Default for Container {
    fn default() -> Self {
        let id = generate_random_string(12);
        let container_root_dir = PathBuf::from(format!("/tmp/faber/{}", id));
        let ro_bind_mounts = vec!["/bin", "/lib", "/lib64", "/usr"];
        Self {
            id,
            container_root_dir,
            ro_bind_mounts,
        }
    }
}

impl Container {
    pub fn setup(&self) -> Result<()> {
        self.create_container_root_dir()?;

        let unshare_flags = CloneFlags::CLONE_NEWUTS // hostname
            | CloneFlags::CLONE_NEWNET // network
            | CloneFlags::CLONE_NEWNS // mount
            | CloneFlags::CLONE_NEWPID; // PID

        unshare(unshare_flags).map_err(|e| FaberError::Unshare { e })?;

        self.remount_root()?;
        self.bind_mounts()?;

        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        remove_dir(&self.container_root_dir)
            .map_err(|e| FaberError::RemoveContainerRootDir { e })?;
        Ok(())
    }

    fn create_container_root_dir(&self) -> Result<()> {
        create_dir_all(&self.container_root_dir)
            .map_err(|e| FaberError::CreateContainerRootDir { e })?;
        Ok(())
    }

    fn remount_root(&self) -> Result<()> {
        // Rebind `/` to make it private
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
            create_dir_all(PathBuf::from(source)).map_err(|e| FaberError::CreateDir { e })?;

            let flags = MsFlags::MS_BIND | MsFlags::MS_RDONLY;
            let target = self.container_root_dir.join(source);
            mount(
                Some(*source),
                target.as_os_str(),
                None::<&str>,
                flags,
                None::<&str>,
            )
            .map_err(|e| FaberError::Mount {
                e,
                details: "Failed to bind mount".to_string(),
            })?;
        }
        Ok(())
    }

    // fn unmount_bind_mounts(&self) -> Result<()> {
    //     for m in &self.ro_bind_mounts {
    //         umount(*m).map_err(|e| FaberError::Unmount {
    //             e,
    //             details: "Failed to unmount bind mount".to_string(),
    //         })?;
    //     }
    //     Ok(())
    // }
}
