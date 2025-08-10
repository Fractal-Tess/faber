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

        let cgroup_mount = Path::new("/sys/fs/cgroup");
        if !cgroup_mount.exists() {
            return Ok(None);
        }

        // Work within the current process's cgroup base to avoid requiring root-level changes
        let self_rel_path = Self::read_self_cgroup_path().unwrap_or_else(|| PathBuf::from("/"));
        let base = cgroup_mount.join(self_rel_path);

        // Ensure base has controllers enabled by moving self into a leaf and enabling subtree_control
        let _ = Self::enable_controllers_on_parent(&base);

        // Ensure a dedicated parent for faber exists under base and has controllers enabled
        let faber_parent = base.join("faber");
        create_dir_all(&faber_parent).map_err(|source| Error::CgroupCreate {
            path: faber_parent.clone(),
            source,
        })?;
        let _ = Self::enable_controllers_on_parent(&faber_parent);

        // Create unique cgroup path for this request using the container_root name
        let group_name = container_root
            .file_name()
            .map(|os| os.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("pid-{child}"));

        // Use a unique path that includes the request ID to avoid conflicts
        let unique_cgroup_path = faber_parent.join(&group_name);
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

        // Emit a best-effort debug snapshot of cgroup state
        self.debug_group_state(&unique_cgroup_path);

        Ok(Some(CgroupHandle {
            path: unique_cgroup_path,
            manager: self.clone(),
        }))
    }

    /// Enable controllers on a parent cgroup directory by first moving the current
    /// process into a dedicated leaf (so the parent has no tasks) and then writing
    /// "+pids +cpu +memory" to its cgroup.subtree_control. Best-effort.
    fn enable_controllers_on_parent(parent: &Path) -> Result<()> {
        // Create manager leaf
        let mgr = parent.join(".mgr");
        let _ = create_dir_all(&mgr);
        // Move current thread group into leaf
        let pid = std::process::id();
        let _ = write(mgr.join("cgroup.procs"), pid.to_string());
        // Enable controllers on parent
        let _ = write(parent.join("cgroup.subtree_control"), b"+pids +cpu +memory");
        Ok(())
    }

    /// Read this process's cgroup v2 relative path from /proc/self/cgroup (0::/path)
    fn read_self_cgroup_path() -> Option<PathBuf> {
        if let Ok(s) = std::fs::read_to_string("/proc/self/cgroup") {
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("0::") {
                    let p = rest.trim();
                    return Some(PathBuf::from(if p.is_empty() { "/" } else { p }));
                }
            }
        }
        None
    }

    /// Best-effort debugging output about cgroup configuration and the created group.
    /// This prints to stderr so callers in applications can capture it in logs if desired.
    pub(crate) fn debug_group_state(&self, group_path: &Path) {
        fn read(path: &Path) -> Option<String> {
            std::fs::read_to_string(path)
                .ok()
                .map(|s| s.trim().to_string())
        }
        fn exists(path: &Path) -> bool {
            path.exists()
        }

        let root = Path::new("/sys/fs/cgroup");
        let self_rel = Self::read_self_cgroup_path().unwrap_or_else(|| PathBuf::from("/"));
        let base = root.join(&self_rel);
        let faber_parent = base.join("faber");

        eprintln!("[cgroups-debug] base: {}", base.display());
        eprintln!(
            "[cgroups-debug] base.controllers: {:?}",
            read(&base.join("cgroup.controllers"))
        );
        eprintln!(
            "[cgroups-debug] base.subtree_control: {:?}",
            read(&base.join("cgroup.subtree_control"))
        );
        eprintln!(
            "[cgroups-debug] faber_parent.subtree_control: {:?}",
            read(&faber_parent.join("cgroup.subtree_control"))
        );
        eprintln!("[cgroups-debug] group: {}", group_path.display());
        eprintln!(
            "[cgroups-debug] has pids.max: {} value: {:?}",
            exists(&group_path.join("pids.max")),
            read(&group_path.join("pids.max"))
        );
        eprintln!(
            "[cgroups-debug] has memory.max: {} value: {:?}",
            exists(&group_path.join("memory.max")),
            read(&group_path.join("memory.max"))
        );
        eprintln!(
            "[cgroups-debug] has cpu.max: {} value: {:?}",
            exists(&group_path.join("cpu.max")),
            read(&group_path.join("cpu.max"))
        );
        eprintln!(
            "[cgroups-debug] procs: {:?}",
            read(&group_path.join("cgroup.procs"))
        );
    }

    pub(crate) fn cleanup_group(&self, group_path: &Path) -> Result<()> {
        remove_dir(group_path).map_err(|source| Error::RemoveDir {
            path: group_path.to_path_buf(),
            source,
        })
    }
}
