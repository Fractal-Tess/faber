use std::env::set_current_dir;
use std::fs::{create_dir_all, remove_dir, remove_dir_all};
use std::path::{Path, PathBuf};

use nix::mount::{MntFlags, MsFlags, mount, umount2};
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

        self.rebind_root()?;
        self.rebind_new_root()?;
        self.bind_mounts()?;
        self.pivot_root()?;

        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        // First, try to unmount the container root directory
        // This will fail if there are still processes using it, but that's expected
        let _ = umount2(&self.container_root_dir, MntFlags::MNT_DETACH);

        // Then remove the directory
        remove_dir_all(&self.container_root_dir).map_err(|e| {
            FaberError::RemoveContainerRootDir {
                e,
                details: "Failed to remove container root directory".to_string(),
            }
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

        // println!("Removing /oldroot directory");
        remove_dir("/oldroot").map_err(|e| FaberError::RemoveDir {
            e,
            details: "Failed to remove old root directory".to_string(),
        })?;

        Ok(())
    }
}
