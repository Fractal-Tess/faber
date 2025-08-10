use crate::runtime::Runtime;
use crate::types::{CgroupConfig, Mount};
use std::path::PathBuf;

#[derive(Default)]
pub struct RuntimeBuilder {
    pub(crate) runtime: Runtime,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            runtime: Runtime::default(),
        }
    }

    pub fn with_mounts(mut self, mounts: Vec<Mount>) -> Self {
        self.runtime.mounts = mounts;
        self
    }

    pub fn with_cgroups(mut self, cfg: CgroupConfig) -> Self {
        self.runtime.cgroup = Some(cfg);
        self
    }

    pub fn with_workdir(mut self, work_dir: String) -> Self {
        self.runtime.work_dir = work_dir;
        self
    }

    pub fn with_container_root(mut self, container_root: impl Into<PathBuf>) -> Self {
        self.runtime.container_root = container_root.into();
        self
    }

    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.runtime.hostname = hostname;
        self
    }

    pub fn build(self) -> Runtime {
        self.runtime
    }
}
