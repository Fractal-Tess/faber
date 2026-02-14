use std::fs::{create_dir_all, read_dir, read_to_string, remove_dir, write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tracing::{debug, warn};

use super::{config::CgroupConfig, task::TaskCgroup};
use crate::prelude::*;

static CGROUP_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CGROUP_INIT_ERROR: Mutex<Option<String>> = Mutex::new(None);
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
        if CGROUP_INITIALIZED.load(Ordering::SeqCst) {
            if let Ok(guard) = CGROUP_INIT_ERROR.lock() {
                if let Some(error) = guard.as_ref() {
                    return Err(FaberError::Generic {
                        message: format!("Cgroup initialization previously failed: {}", error),
                    });
                }
            }
            return Ok(());
        }

        if let Err(e) = Self::create_faber_cgroup_hierarchy() {
            if let Ok(mut guard) = CGROUP_INIT_ERROR.lock() {
                *guard = Some(e.to_string());
            }
            return Err(e);
        }

        CGROUP_INITIALIZED.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Returns the resolved faber cgroup path (e.g. /sys/fs/cgroup/.../faber).
    /// Must be called after ensure_faber_cgroup_hierarchy().
    pub fn get_faber_cgroup_path() -> Result<PathBuf> {
        if let Ok(guard) = FABER_CGROUP_PATH.lock() {
            if let Some(path) = guard.as_ref() {
                return Ok(path.clone());
            }
        }
        Err(FaberError::Generic {
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

    pub fn create_faber_cgroup_hierarchy() -> Result<()> {
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

        // Cache the resolved path for TaskCgroup to use
        if let Ok(mut guard) = FABER_CGROUP_PATH.lock() {
            *guard = Some(faber_cgroup_path.clone());
        }

        debug!(
            "Faber cgroup hierarchy created at {}",
            faber_cgroup_path.display()
        );

        Ok(())
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

    pub fn create_task_cgroup(&self) -> Result<TaskCgroup> {
        TaskCgroup::new(self.config.clone())
    }
}
