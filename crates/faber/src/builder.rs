use crate::cgroups::Cgroups;
use crate::environment::ContainerEnvironment;
use crate::runtime::Runtime;
use crate::types::{CgroupConfig, Mount};
use rand::{Rng, distr::Alphanumeric};
use std::path::PathBuf;

#[derive(Default)]
pub struct RuntimeBuilder {
    container_root: Option<PathBuf>,
    hostname: Option<String>,
    mounts: Option<Vec<Mount>>,
    work_dir: Option<String>,
    cgroup: Option<CgroupConfig>,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_mounts(mut self, mounts: Vec<Mount>) -> Self {
        self.mounts = Some(mounts);
        self
    }

    pub fn with_cgroups(mut self, cfg: CgroupConfig) -> Self {
        self.cgroup = Some(cfg);
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

    pub fn build(self) -> Runtime {
        // Defaults mirror Runtime::default()
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

        let id: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        let container_root = self
            .container_root
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/faber/containers/{id}")));
        let hostname = self.hostname.unwrap_or_else(|| "faber".into());
        let mounts = self.mounts.unwrap_or(default_mounts);
        let work_dir = self.work_dir.unwrap_or_else(|| "/faber".into());

        let env = ContainerEnvironment::new(container_root, hostname, mounts, work_dir);
        let cgroups = Cgroups::new(self.cgroup);
        Runtime { env, cgroups }
    }
}
