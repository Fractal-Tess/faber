use crate::prelude::*;
use crate::runtime::Runtime;
use crate::types::{CgroupConfig, FilesystemConfig, Mount, RuntimeLimits};
use nix::mount::MsFlags;
use rand::Rng;
use std::path::PathBuf;

/// Builder for constructing a `Runtime` with clear, typed configuration.
///
/// Use fluent methods to customize the container root, hostname, bind mounts,
/// working directory, and filesystem sizes. Call [`build`](Self::build) to
/// produce a ready-to-run [`Runtime`].
#[derive(Default)]
pub struct RuntimeBuilder {
    container_root: Option<PathBuf>,
    hostname: Option<String>,
    mounts: Option<Vec<Mount>>,
    work_dir: Option<String>,
    filesystem_config: Option<FilesystemConfig>,
    cgroup: Option<CgroupConfig>,

    id: Option<String>,

    runtime_limits: Option<RuntimeLimits>,
}

impl RuntimeBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Provide custom bind mounts.
    pub fn with_mounts(mut self, mounts: Vec<Mount>) -> Self {
        self.mounts = Some(mounts);
        self
    }

    /// Set an explicit runtime identifier.
    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the working directory path inside the container view.
    pub fn with_workdir(mut self, work_dir: String) -> Self {
        self.work_dir = Some(work_dir);
        self
    }

    /// Set the host path that will become the container root.
    pub fn with_container_root(mut self, container_root: impl Into<PathBuf>) -> Self {
        self.container_root = Some(container_root.into());
        self
    }

    /// Set the container hostname (UTS namespace).
    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.hostname = Some(hostname);
        self
    }

    /// Sets the filesystem configuration for tmp and workdir sizes.
    pub fn with_filesystem_config(
        mut self,
        tmp_size: impl Into<String>,
        workdir_size: impl Into<String>,
    ) -> Self {
        self.filesystem_config = Some(FilesystemConfig {
            tmp_size: tmp_size.into(),
            workdir_size: workdir_size.into(),
        });
        self
    }

    /// Sets the tmp filesystem size.
    pub fn with_tmp_size(mut self, tmp_size: impl Into<String>) -> Self {
        let mut config = self.filesystem_config.unwrap_or_default();
        config.tmp_size = tmp_size.into();
        self.filesystem_config = Some(config);
        self
    }

    /// Sets the workdir filesystem size.
    pub fn with_workdir_size(mut self, workdir_size: impl Into<String>) -> Self {
        let mut config = self.filesystem_config.unwrap_or_default();
        config.workdir_size = workdir_size.into();
        self.filesystem_config = Some(config);
        self
    }

    /// Configure cgroup limits.
    pub fn with_cgroup_config(mut self, cfg: CgroupConfig) -> Self {
        self.cgroup = Some(cfg);
        self
    }

    /// Configure runtime limits.
    pub fn with_kill_timeout_seconds(mut self, kill_timeout_seconds: Option<u64>) -> Self {
        let mut limits = self.runtime_limits.unwrap_or_default();
        limits.kill_timeout_seconds = kill_timeout_seconds;
        self.runtime_limits = Some(limits);
        self
    }

    /// Configure CPU time limit in milliseconds.
    pub fn with_cpu_time_limit_ms(mut self, cpu_time_limit_ms: Option<u64>) -> Self {
        let mut limits = self.runtime_limits.unwrap_or_default();
        limits.cpu_time_limit_ms = cpu_time_limit_ms;
        self.runtime_limits = Some(limits);
        self
    }

    /// Finalize the configuration and create a [`Runtime`].
    pub fn build(self) -> Result<Runtime> {
        // Validate required fields
        if let Some(ref mounts) = self.mounts {
            for mount in mounts {
                if mount.source.is_empty() {
                    return Err(Error::Validation {
                        field: "mount source".to_string(),
                        details: "Mount source cannot be empty".to_string(),
                    });
                }
                if mount.target.is_empty() {
                    return Err(Error::Validation {
                        field: "mount target".to_string(),
                        details: "Mount target cannot be empty".to_string(),
                    });
                }
            }
        }

        let flags = vec![MsFlags::MS_BIND, MsFlags::MS_REC, MsFlags::MS_RDONLY];
        let default_mounts: Vec<Mount> = ["/bin", "/lib", "/usr", "/lib64", "/etc"]
            .iter()
            .map(|s| Mount {
                source: s.to_string(),
                target: s.to_string(),
                flags: flags.clone(),
                options: vec![],
                data: None,
            })
            .collect();

        let id: String = self.id.unwrap_or_else(|| {
            let mut rng = rand::thread_rng();
            std::iter::repeat(())
                .map(|_| rng.sample(rand::distributions::Alphanumeric))
                .take(12)
                .map(char::from)
                .collect()
        });
        let container_root = self
            .container_root
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/faber/containers/{id}")));
        let hostname = self.hostname.unwrap_or_else(|| "faber".into());
        let mounts = self.mounts.unwrap_or(default_mounts);
        let work_dir = self.work_dir.unwrap_or_else(|| "/faber".into());
        let filesystem_config = self.filesystem_config.unwrap_or_default();
        let runtime_limits = self.runtime_limits.unwrap_or_default();
        let cgroup_config = self.cgroup.unwrap_or_default();

        // Validate work_dir
        if work_dir.is_empty() {
            return Err(Error::Validation {
                field: "work_dir".to_string(),
                details: "Work directory cannot be empty".to_string(),
            });
        }

        Ok(Runtime {
            host_container_root: container_root,
            hostname,
            mounts,
            work_dir: work_dir.into(),
            filesystem_config,
            runtime_limits,
            cgroup_config,
        })
    }
}
