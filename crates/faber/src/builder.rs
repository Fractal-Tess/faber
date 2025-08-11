use crate::cgroup::CgroupManager;
use crate::environment::ContainerEnvironment;
use crate::prelude::*;
use crate::runtime::Runtime;
use crate::types::{CgroupConfig, Mount, RuntimeLimits};
use rand::{Rng, distr::Alphanumeric};
use std::path::PathBuf;

#[derive(Default)]
pub struct RuntimeBuilder {
    container_root: Option<PathBuf>,
    hostname: Option<String>,
    mounts: Option<Vec<Mount>>,
    work_dir: Option<String>,
    limits: Option<RuntimeLimits>,

    id: Option<String>,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mounts(mut self, mounts: Vec<Mount>) -> Self {
        self.mounts = Some(mounts);
        self
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_workdir(mut self, work_dir: String) -> Self {
        self.work_dir = Some(work_dir);
        self
    }
    pub fn with_container_root(mut self, container_root: impl Into<PathBuf>) -> Self {
        self.container_root = Some(container_root.into());
        self
    }
    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.hostname = Some(hostname);
        self
    }
    pub fn with_runtime_limits(mut self, limits: RuntimeLimits) -> Self {
        self.limits = Some(limits);
        self
    }

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

        let flags = vec![
            nix::mount::MsFlags::MS_BIND,
            nix::mount::MsFlags::MS_REC,
            nix::mount::MsFlags::MS_RDONLY,
        ];
        let default_mounts: Vec<Mount> = ["/bin", "/lib", "/usr", "/lib64", "/sbin"]
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
            rand::rng()
                .sample_iter(&Alphanumeric)
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

        // Validate work_dir
        if work_dir.is_empty() {
            return Err(Error::Validation {
                field: "work_dir".to_string(),
                details: "Work directory cannot be empty".to_string(),
            });
        }

        let env = ContainerEnvironment::new(container_root, hostname, mounts, work_dir);
        let limits = self.limits.unwrap_or_default();

        Ok(Runtime { id, env, limits })
    }
}
