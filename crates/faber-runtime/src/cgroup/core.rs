use std::fs::{create_dir_all, write};
use std::path::PathBuf;
use std::sync::Once;

use super::{config::CgroupConfig, task::TaskCgroup};
use crate::prelude::*;

static CGROUP_HIERARCHY_INIT: Once = Once::new();

#[derive(Debug, Clone, Default)]
pub struct Cgroup {
    config: CgroupConfig,
}

impl Cgroup {
    pub fn new(config: CgroupConfig) -> Self {
        Self { config }
    }

    pub fn ensure_faber_cgroup_hierarchy() -> Result<()> {
        CGROUP_HIERARCHY_INIT.call_once(|| {
            if let Err(e) = Self::create_faber_cgroup_hierarchy() {
                eprintln!("Failed to create faber cgroup hierarchy: {}", e);
            }
        });
        Ok(())
    }

    pub fn create_faber_cgroup_hierarchy() -> Result<()> {
        let controllers = "+cpu +memory +pids";
        let cgroup_path = PathBuf::from("/sys/fs/cgroup");

        let root_subtree_control_path = cgroup_path.join("cgroup.subtree_control");
        write(root_subtree_control_path, controllers).map_err(|e| {
            FaberError::CgroupControllers {
                e,
                details: "Failed to set controllers in cgroup.subtree_control in root cgroup"
                    .to_string(),
            }
        })?;

        let faber_cgroup_path = cgroup_path.join("faber");
        create_dir_all(&faber_cgroup_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed tocreate faber cgroup directory".to_string(),
        })?;

        let faber_subtree_control = faber_cgroup_path.join("cgroup.subtree_control");
        write(&faber_subtree_control, "+cpu +memory +pids")
            .or_else(|e| {
                if e.raw_os_error() == Some(16) {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .map_err(|e| FaberError::CgroupControllerEnable {
                e,
                details: "Failed to set controllers in cgroup.subtree_control in faber cgroup"
                    .to_string(),
            })?;

        Ok(())
    }

    pub fn create_task_cgroup(&self) -> Result<TaskCgroup> {
        TaskCgroup::new(self.config.clone())
    }
}
