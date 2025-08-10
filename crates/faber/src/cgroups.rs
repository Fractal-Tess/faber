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

impl CgroupHandle {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
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
        if let Some(cfg) = &self.config {
            if !cfg.enabled {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }

        let cgroup_root = Path::new("/sys/fs/cgroup");
        if !cgroup_root.exists() {
            return Ok(None);
        }

        // Ensure parent namespace for faber exists and enable controllers up the tree
        let faber_root = cgroup_root.join("faber");
        create_dir_all(&faber_root).map_err(|source| Error::CgroupCreate {
            path: faber_root.clone(),
            source,
        })?;

        // Best-effort: enable controllers at the root and parent to allow limits on children
        let _ = write(
            cgroup_root.join("cgroup.subtree_control"),
            b"+pids +cpu +memory",
        );
        let _ = write(
            faber_root.join("cgroup.subtree_control"),
            b"+pids +cpu +memory",
        );

        // Create unique cgroup path for this request using the container_root name
        let group_name = container_root
            .file_name()
            .map(|os| os.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("pid-{child}"));

        // Use a unique path that includes the request ID to avoid conflicts
        let unique_cgroup_path = faber_root.join(&group_name);
        create_dir_all(&unique_cgroup_path).map_err(|source| Error::CgroupCreate {
            path: unique_cgroup_path.clone(),
            source,
        })?;

        // Apply configured limits where provided (best-effort)
        if let Some(cfg) = &self.config {
            if let Some(v) = &cfg.pids_max {
                let _ = write(unique_cgroup_path.join("pids.max"), v);
            }
            if let Some(v) = &cfg.memory_max {
                let _ = write(unique_cgroup_path.join("memory.max"), v);
            }
            if let Some(v) = &cfg.cpu_max {
                let _ = write(unique_cgroup_path.join("cpu.max"), v);
            }
        }

        let procs_file = unique_cgroup_path.join("cgroup.procs");
        write(&procs_file, child.as_raw().to_string()).map_err(|source| Error::CgroupWrite {
            path: procs_file.clone(),
            value: child.as_raw().to_string(),
            source,
        })?;

        Ok(Some(CgroupHandle {
            path: unique_cgroup_path,
            manager: self.clone(),
        }))
    }

    pub(crate) fn cleanup_group(&self, group_path: &Path) -> Result<()> {
        remove_dir(group_path).map_err(|source| Error::RemoveDir {
            path: group_path.to_path_buf(),
            source,
        })
    }
}
