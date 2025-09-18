use std::path::PathBuf;

use super::config::ContainerConfig;

#[derive(Default)]
pub struct ContainerConfigBuilder {
    config: ContainerConfig,
}

impl ContainerConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: ContainerConfig::default(),
        }
    }

    pub fn with_ro_bind_mounts(mut self, ro_bind_mounts: Vec<&'static str>) -> Self {
        self.config.bind_mounts_ro = ro_bind_mounts;
        self
    }

    pub fn with_w_bind_mounts(mut self, w_bind_mounts: Vec<&'static str>) -> Self {
        self.config.bind_mounts_rw = w_bind_mounts;
        self
    }

    pub fn with_tmpdir_size(mut self, tmpdir_size: String) -> Self {
        self.config.tmpdir_size = tmpdir_size;
        self
    }

    pub fn with_workdir_size(mut self, workdir_size: String) -> Self {
        self.config.workdir_size = workdir_size;
        self
    }

    pub fn with_workdir(mut self, workdir: PathBuf) -> Self {
        self.config.workdir = workdir;
        self
    }

    pub fn with_hostname(mut self, hostname: String) -> Self {
        self.config.hostname = hostname;
        self
    }

    pub fn build(self) -> ContainerConfig {
        self.config
    }
}
