use std::fs::{create_dir_all, read_dir, read_to_string, remove_dir, write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{debug, warn};

use super::{config::CgroupConfig, task::{TaskCgroup, parse_memory_string}};
use crate::prelude::*;

static FABER_CGROUP_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug, Clone, Default)]
pub struct Cgroup {
    config: CgroupConfig,
}

impl Cgroup {
    pub fn new(config: CgroupConfig) -> Self {
        Self { config }
    }

    pub fn ensure_faber_cgroup_hierarchy() -> Result<()> {
        let mut path = FABER_CGROUP_PATH.lock().map_err(|_| FaberError::Generic {
            message: "Faber cgroup initialization lock was poisoned".to_string(),
        })?;
        if path.is_some() {
            return Ok(());
        }

        *path = Some(Self::create_faber_cgroup_hierarchy()?);
        Ok(())
    }

    /// Returns the resolved faber cgroup path (e.g. /sys/fs/cgroup/.../faber).
    /// Must be called after ensure_faber_cgroup_hierarchy().
    pub fn get_faber_cgroup_path() -> Result<PathBuf> {
        FABER_CGROUP_PATH
            .lock()
            .map_err(|_| FaberError::Generic {
                message: "Faber cgroup initialization lock was poisoned".to_string(),
            })?
            .clone()
            .ok_or_else(|| FaberError::Generic {
                message: "Faber cgroup hierarchy not initialized".to_string(),
            })
    }

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

    fn enable_controllers(path: &PathBuf, context: &str) -> Result<()> {
        if Self::controllers_already_enabled(path) {
            return Ok(());
        }

        write(path, "+cpu +memory +pids")
            .or_else(|e| {
                if e.raw_os_error() == Some(16) {
                    return Ok(());
                }
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

    /// Detect the current process's cgroup v2 path on the filesystem.
    /// Reads /proc/self/cgroup to find "0::/<relative-path>" and resolves it
    /// against the cgroup2 mount at /sys/fs/cgroup.
    fn detect_own_cgroup_path() -> Result<PathBuf> {
        let content = read_to_string("/proc/self/cgroup").map_err(|e| FaberError::Generic {
            message: format!("Failed to read /proc/self/cgroup: {}", e),
        })?;

        for line in content.lines() {
            if let Some(rel_path) = line.strip_prefix("0::") {
                let rel_path = rel_path.trim_start_matches('/');
                let full_path = if rel_path.is_empty() {
                    PathBuf::from("/sys/fs/cgroup")
                } else {
                    PathBuf::from("/sys/fs/cgroup").join(rel_path)
                };
                debug!("Detected own cgroup path: {}", full_path.display());
                return Ok(full_path);
            }
        }

        debug!("Could not parse /proc/self/cgroup, falling back to /sys/fs/cgroup");
        Ok(PathBuf::from("/sys/fs/cgroup"))
    }

    fn create_faber_cgroup_hierarchy() -> Result<PathBuf> {
        let base_cgroup_path = Self::detect_own_cgroup_path()?;
        let subtree_control_path = base_cgroup_path.join("cgroup.subtree_control");

        // Enable controllers on the base cgroup. If processes are present directly
        // in this cgroup (the "no internal processes" rule), move them to an init
        // child cgroup first.
        if !Self::controllers_already_enabled(&subtree_control_path) {
            let init_cgroup_path = base_cgroup_path.join("faber-init");
            create_dir_all(&init_cgroup_path).map_err(|e| FaberError::CreateDir {
                e,
                details: "Failed to create faber-init cgroup directory".to_string(),
            })?;

            let root_procs =
                read_to_string(base_cgroup_path.join("cgroup.procs")).unwrap_or_default();
            let init_procs_path = init_cgroup_path.join("cgroup.procs");
            for pid in root_procs.lines().filter(|l| !l.is_empty()) {
                let _ = write(&init_procs_path, pid);
            }

            Self::enable_controllers(
                &subtree_control_path,
                "cgroup.subtree_control in base cgroup",
            )?;
        }

        let faber_cgroup_path = base_cgroup_path.join("faber");
        create_dir_all(&faber_cgroup_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create faber cgroup directory".to_string(),
        })?;

        Self::cleanup_stale_task_cgroups(&faber_cgroup_path);

        let faber_subtree_control = faber_cgroup_path.join("cgroup.subtree_control");
        Self::enable_controllers(
            &faber_subtree_control,
            "cgroup.subtree_control in faber cgroup",
        )?;

        Self::setup_faber_cgroup_limits(&faber_cgroup_path)?;

        debug!(
            "Faber cgroup hierarchy created at {}",
            faber_cgroup_path.display()
        );

        Ok(faber_cgroup_path)
    }

    fn cleanup_stale_task_cgroups(faber_cgroup_path: &PathBuf) {
        if let Ok(entries) = read_dir(faber_cgroup_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name() {
                        if name.to_string_lossy().starts_with("task-") {
                            let procs_path = path.join("cgroup.procs");
                            if let Ok(procs) = read_to_string(&procs_path) {
                                if procs.trim().is_empty() {
                                    if let Err(e) = remove_dir(&path) {
                                        warn!(
                                            "Failed to cleanup stale cgroup {}: {}",
                                            path.display(),
                                            e
                                        );
                                    } else {
                                        debug!("Cleaned up stale cgroup: {}", path.display());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn setup_faber_cgroup_limits(faber_cgroup_path: &PathBuf) -> Result<()> {
        let memory_max_path = faber_cgroup_path.join("memory.max");
        if let Err(e) = write(&memory_max_path, "max") {
            debug!(
                "Could not set memory.max on faber cgroup (non-critical): {}",
                e
            );
        }

        let cpu_max_path = faber_cgroup_path.join("cpu.max");
        if let Err(e) = write(&cpu_max_path, "max 100000") {
            debug!(
                "Could not set cpu.max on faber cgroup (non-critical): {}",
                e
            );
        }

        Ok(())
    }

    pub fn configure_service_limits(
        per_task_memory: &str,
        per_task_pids: u32,
        max_concurrency: usize,
    ) -> Result<()> {
        let path = Self::get_faber_cgroup_path()?;
        let memory = parse_memory_string(per_task_memory)?
            .checked_mul(max_concurrency as u64)
            .ok_or_else(|| FaberError::Generic {
                message: "Aggregate service memory limit overflows u64".to_string(),
            })?;
        let pids = u64::from(per_task_pids)
            .checked_mul(max_concurrency as u64)
            .ok_or_else(|| FaberError::Generic {
                message: "Aggregate service PID limit overflows u64".to_string(),
            })?;

        write(path.join("memory.max"), memory.to_string()).map_err(|e| {
            FaberError::WriteFile {
                e,
                details: "Failed to set aggregate service memory limit".to_string(),
            }
        })?;
        write(path.join("memory.swap.max"), "0").map_err(|e| FaberError::WriteFile {
            e,
            details: "Failed to disable aggregate service swap".to_string(),
        })?;
        write(path.join("pids.max"), pids.to_string()).map_err(|e| FaberError::WriteFile {
            e,
            details: "Failed to set aggregate service PID limit".to_string(),
        })?;
        Ok(())
    }

    pub fn kill_active_tasks() -> Result<()> {
        let path = Self::get_faber_cgroup_path()?;
        let mut task_paths = Vec::new();
        for entry in read_dir(path).map_err(|e| FaberError::Generic {
            message: format!("Failed to enumerate active task cgroups: {e}"),
        })? {
            let entry = entry.map_err(|e| FaberError::Generic {
                message: format!("Failed to inspect active task cgroup: {e}"),
            })?;
            if entry.file_type().is_ok_and(|kind| kind.is_dir())
                && entry.file_name().to_string_lossy().starts_with("task-")
            {
                task_paths.push(entry.path());
                let kill_path = entry.path().join("cgroup.kill");
                if let Err(error) = write(&kill_path, "1")
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    return Err(FaberError::WriteFile {
                        e: error,
                        details: format!("Failed to kill task cgroup at {}", kill_path.display()),
                    });
                }
            }
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
        for task_path in task_paths {
            while std::time::Instant::now() < deadline {
                let populated = read_to_string(task_path.join("cgroup.events"))
                    .is_ok_and(|contents| contents.lines().any(|line| line == "populated 1"));
                if !populated {
                    if let Err(error) = remove_dir(&task_path)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        debug!(path = %task_path.display(), %error, "task cgroup cleanup deferred");
                    }
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        Ok(())
    }

    pub fn create_task_cgroup(&self, faber_cgroup_path: &Path) -> Result<TaskCgroup> {
        TaskCgroup::new(self.config.clone(), faber_cgroup_path)
    }
}
