use std::fs::{create_dir_all, read_dir, read_to_string, write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing::{debug, warn};

use super::{
    config::CgroupConfig,
    request::{self, REQUEST_CGROUP_PREFIX, RequestCgroup, RequestLimits},
    task::{TaskCgroup, disable_swap, parse_memory_string},
};
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

    pub(crate) fn config(&self) -> &CgroupConfig {
        &self.config
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

    /// Resolve the cgroup this service manages: the one it was started in.
    /// Reads /proc/self/cgroup to find "0::/<relative-path>" and resolves it
    /// against the cgroup2 mount at /sys/fs/cgroup.
    fn detect_own_cgroup_path() -> Result<PathBuf> {
        let content = read_to_string("/proc/self/cgroup").map_err(|e| FaberError::Generic {
            message: format!("Failed to read /proc/self/cgroup: {}", e),
        })?;
        let relative_path = content
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .ok_or_else(|| FaberError::Generic {
                message: "This process is not in a cgroup v2 hierarchy".to_string(),
            })?;
        let own_path = PathBuf::from("/sys/fs/cgroup").join(relative_path.trim_start_matches('/'));

        // /proc/self/cgroup is relative to the cgroup namespace while the
        // mount may be the host's whole tree. When the two disagree the
        // resolved path is someone else's cgroup (the host root, for a
        // private namespace), so refuse to manage it.
        let members =
            read_to_string(own_path.join("cgroup.procs")).map_err(|e| FaberError::Generic {
                message: format!("Failed to read {}/cgroup.procs: {}", own_path.display(), e),
            })?;
        if !Self::lists_process(&members, std::process::id()) {
            return Err(FaberError::Generic {
                message: format!(
                    "{} does not contain this process: the cgroup mount and the cgroup \
                     namespace disagree. Run the container with --cgroupns=host and the \
                     host's /sys/fs/cgroup mounted, or with neither.",
                    own_path.display()
                ),
            });
        }

        let base_path = Self::service_base(&own_path).to_path_buf();
        debug!("Detected base cgroup path: {}", base_path.display());
        Ok(base_path)
    }

    fn lists_process(cgroup_procs: &str, pid: u32) -> bool {
        cgroup_procs
            .lines()
            .any(|line| line.trim().parse::<u32>() == Ok(pid))
    }

    /// A previous start moves the service into `<base>/faber-init`, and a
    /// restart in the same container begins there. Step back out so the
    /// hierarchy is reused instead of nesting one level deeper per restart.
    fn service_base(own_path: &Path) -> &Path {
        match (own_path.file_name(), own_path.parent()) {
            (Some(name), Some(parent)) if name == "faber-init" => parent,
            _ => own_path,
        }
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

    fn cleanup_stale_task_cgroups(faber_cgroup_path: &Path) {
        for path in Self::sandbox_cgroups(faber_cgroup_path) {
            if request::is_populated(&path) {
                continue;
            }
            match request::remove_cgroup_tree(&path) {
                Ok(()) => debug!("Cleaned up stale cgroup: {}", path.display()),
                Err(e) => warn!("Failed to cleanup stale cgroup {}: {}", path.display(), e),
            }
        }
    }

    /// Top-level request and legacy task cgroups beneath the Faber cgroup.
    fn sandbox_cgroups(faber_cgroup_path: &Path) -> Vec<PathBuf> {
        read_dir(faber_cgroup_path)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && (name.starts_with(REQUEST_CGROUP_PREFIX) || name.starts_with("task-"))
            })
            .map(|entry| entry.path())
            .collect()
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

    /// Size the Faber cgroup for `task_slots` concurrently running tasks.
    /// Every running request holds at least one slot, so there are at most
    /// `task_slots` workspaces as well.
    pub fn configure_service_limits(
        per_task_memory: &str,
        per_task_pids: u32,
        task_slots: usize,
        workspace_allowance: u64,
    ) -> Result<()> {
        let path = Self::get_faber_cgroup_path()?;
        let memory = parse_memory_string(per_task_memory)?
            .checked_add(workspace_allowance)
            .and_then(|per_slot| per_slot.checked_mul(task_slots as u64))
            .ok_or_else(|| FaberError::Generic {
                message: "Aggregate service memory limit overflows u64".to_string(),
            })?;
        let pids = u64::from(per_task_pids)
            .checked_mul(task_slots as u64)
            .ok_or_else(|| FaberError::Generic {
                message: "Aggregate service PID limit overflows u64".to_string(),
            })?;

        write(path.join("memory.max"), memory.to_string()).map_err(|e| FaberError::WriteFile {
            e,
            details: "Failed to set aggregate service memory limit".to_string(),
        })?;
        disable_swap(&path.join("memory.swap.max")).map_err(|e| FaberError::WriteFile {
            e,
            details: "Failed to disable aggregate service swap".to_string(),
        })?;
        write(path.join("pids.max"), pids.to_string()).map_err(|e| FaberError::WriteFile {
            e,
            details: "Failed to set aggregate service PID limit".to_string(),
        })?;
        Ok(())
    }

    /// Kill every sandbox managed by this Faber cgroup. Service shutdown only:
    /// individual requests kill their own request cgroup instead.
    pub fn kill_active_tasks() -> Result<()> {
        let path = Self::get_faber_cgroup_path()?;
        let sandboxes = Self::sandbox_cgroups(&path);
        for sandbox in &sandboxes {
            request::kill_cgroup_tree(sandbox);
        }
        for sandbox in sandboxes {
            if let Err(error) = request::remove_cgroup_tree(&sandbox) {
                debug!(path = %sandbox.display(), %error, "sandbox cgroup cleanup deferred");
            }
        }
        Ok(())
    }

    /// Create the cgroup for one request whose widest step runs `width` tasks
    /// at once. Its limits cover that many tasks at their per-task limits plus
    /// the request's workspace tmpfs, whose pages are recharged to the request
    /// cgroup once the task that wrote them is gone.
    pub(crate) fn create_request_cgroup(
        &self,
        faber_cgroup_path: &Path,
        width: usize,
        workspace_allowance: Option<u64>,
    ) -> Result<RequestCgroup> {
        let width = width.max(1) as u64;
        let memory_max = match self.config.memory_max.trim() {
            "max" => None,
            memory => parse_memory_string(memory)?
                .checked_mul(width)
                .zip(workspace_allowance)
                .and_then(|(tasks, workspace)| tasks.checked_add(workspace)),
        };
        let pids_max = u64::from(self.config.pids_max).checked_mul(width);
        RequestCgroup::new(
            faber_cgroup_path,
            RequestLimits {
                memory_max,
                pids_max,
            },
        )
    }

    pub fn create_task_cgroup(&self, request_cgroup_path: &Path) -> Result<TaskCgroup> {
        TaskCgroup::new(self.config.clone(), request_cgroup_path)
    }
}

#[cfg(test)]
mod tests {
    use super::Cgroup;
    use std::path::Path;

    #[test]
    fn restarted_service_reuses_the_base_cgroup() {
        let scope = Path::new("/sys/fs/cgroup/system.slice/docker-abc.scope");
        assert_eq!(Cgroup::service_base(scope), scope);
        assert_eq!(Cgroup::service_base(&scope.join("faber-init")), scope);
        assert_eq!(
            Cgroup::service_base(Path::new("/sys/fs/cgroup")),
            Path::new("/sys/fs/cgroup")
        );
    }

    #[test]
    fn membership_requires_an_exact_pid_line() {
        assert!(Cgroup::lists_process("1\n28\n", 28));
        assert!(!Cgroup::lists_process("1\n280\n0\n", 28));
        assert!(!Cgroup::lists_process("", 28));
    }
}
