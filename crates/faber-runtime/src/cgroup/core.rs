use std::fs::{create_dir_all, read_to_string, write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{config::CgroupConfig, task::TaskCgroup};
use crate::prelude::*;

static CGROUP_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default)]
pub struct Cgroup {
    config: CgroupConfig,
}

impl Cgroup {
    pub fn new(config: CgroupConfig) -> Self {
        Self { config }
    }

    pub fn ensure_faber_cgroup_hierarchy() -> Result<()> {
        if CGROUP_INITIALIZED.load(Ordering::SeqCst) {
            return Ok(());
        }
        Self::create_faber_cgroup_hierarchy()?;
        CGROUP_INITIALIZED.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Check if all required controllers are already enabled in a subtree_control file
    fn controllers_already_enabled(subtree_control_path: &PathBuf) -> bool {
        let required = ["cpu", "memory", "pids"];
        match read_to_string(subtree_control_path) {
            Ok(content) => {
                let enabled: Vec<&str> = content.trim().split_whitespace().collect();
                required.iter().all(|r| enabled.contains(r))
            }
            Err(_) => false,
        }
    }

    /// Try to enable controllers, tolerating EBUSY (16) and permission errors
    /// when controllers are already enabled
    fn enable_controllers(path: &PathBuf, context: &str) -> Result<()> {
        if Self::controllers_already_enabled(path) {
            return Ok(());
        }

        write(path, "+cpu +memory +pids")
            .or_else(|e| {
                // EBUSY (16): controllers already in use by children
                if e.raw_os_error() == Some(16) {
                    return Ok(());
                }
                // Permission denied but controllers are already enabled — safe to proceed
                if e.raw_os_error() == Some(13) && Self::controllers_already_enabled(path) {
                    return Ok(());
                }
                Err(e)
            })
            .map_err(|e| FaberError::CgroupControllers {
                e,
                details: format!("Failed to set controllers in {}", context),
            })
    }

    pub fn create_faber_cgroup_hierarchy() -> Result<()> {
        let cgroup_path = PathBuf::from("/sys/fs/cgroup");
        let root_subtree_control_path = cgroup_path.join("cgroup.subtree_control");

        if !Self::controllers_already_enabled(&root_subtree_control_path) {
            // cgroups v2 "no internal process" constraint: must move all processes
            // out of the root cgroup before enabling subtree_control
            let init_cgroup_path = cgroup_path.join("faber-init");
            create_dir_all(&init_cgroup_path).map_err(|e| FaberError::CreateDir {
                e,
                details: "Failed to create faber-init cgroup directory".to_string(),
            })?;

            let root_procs = read_to_string(cgroup_path.join("cgroup.procs")).unwrap_or_default();
            let init_procs_path = init_cgroup_path.join("cgroup.procs");
            for pid in root_procs.lines().filter(|l| !l.is_empty()) {
                let _ = write(&init_procs_path, pid);
            }

            Self::enable_controllers(
                &root_subtree_control_path,
                "cgroup.subtree_control in root cgroup",
            )?;
        }

        let faber_cgroup_path = cgroup_path.join("faber");
        create_dir_all(&faber_cgroup_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create faber cgroup directory".to_string(),
        })?;

        let faber_subtree_control = faber_cgroup_path.join("cgroup.subtree_control");
        Self::enable_controllers(
            &faber_subtree_control,
            "cgroup.subtree_control in faber cgroup",
        )?;

        Ok(())
    }

    pub fn create_task_cgroup(&self) -> Result<TaskCgroup> {
        TaskCgroup::new(self.config.clone())
    }
}
