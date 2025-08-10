use crate::prelude::*;
use crate::types::CgroupConfig;
use nix::unistd::Pid;
use std::{
    fs::{create_dir_all, remove_dir, write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct Cgroups {
    pub(crate) config: Option<CgroupConfig>,
}

#[derive(Debug, Clone)]
pub(crate) struct CgroupHandle {
    path: PathBuf,
    manager: Cgroups,
}

impl Drop for CgroupHandle {
    fn drop(&mut self) {
        let _ = remove_dir(&self.path);
    }
}

impl Cgroups {
    pub(crate) fn new(config: Option<CgroupConfig>) -> Self {
        Self { config }
    }

    pub(crate) fn assign_child(
        &self,
        child: Pid,
        container_root: &Path,
    ) -> Result<Option<CgroupHandle>> {
        // Respect config enabled flag; default to enabled when Some but missing field
        if let Some(cfg) = &self.config {
            if !cfg.enabled {
                return Ok(None);
            }
        } else {
            // No config present: treat as disabled for explicitness per requirement
            return Ok(None);
        }

        let cgroup_root = Path::new("/sys/fs/cgroup");
        if !cgroup_root.exists() {
            return Ok(None);
        }

        let faber_base = cgroup_root.join("faber");
        let _ = create_dir_all(&faber_base);

        let subtree_control = faber_base.join("cgroup.subtree_control");
        let _ = write(&subtree_control, b"+pids +cpu +memory");

        let group_name = container_root
            .file_name()
            .map(|os| os.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("pid-{child}"));
        let group_path = faber_base.join(group_name);
        create_dir_all(&group_path)?;

        if let Some(cfg) = &self.config {
            if let Some(v) = &cfg.pids_max {
                let _ = write(group_path.join("pids.max"), v);
            }
            if let Some(v) = &cfg.memory_max {
                let _ = write(group_path.join("memory.max"), v);
            }
            if let Some(v) = &cfg.cpu_max {
                let _ = write(group_path.join("cpu.max"), v);
            }
        }

        let procs_file = group_path.join("cgroup.procs");
        write(&procs_file, child.as_raw().to_string())?;

        Ok(Some(CgroupHandle {
            path: group_path,
            manager: self.clone(),
        }))
    }

    pub(crate) fn cleanup_group(&self, group_path: &Path) {
        let _ = remove_dir(group_path);
    }
}
