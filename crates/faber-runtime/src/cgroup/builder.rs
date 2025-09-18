use super::config::CgroupConfig;

#[derive(Default)]
pub struct CgroupConfigBuilder {
    config: CgroupConfig,
}

impl CgroupConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: CgroupConfig::default(),
        }
    }

    pub fn with_cpu(mut self, cpu_max: String) -> Self {
        self.config.cpu_max = cpu_max;
        self
    }

    pub fn with_memory(mut self, memory_max: String) -> Self {
        self.config.memory_max = memory_max;
        self
    }

    pub fn with_pids(mut self, pids_max: u32) -> Self {
        self.config.pids_max = pids_max;
        self
    }

    pub fn build(self) -> CgroupConfig {
        self.config
    }
}
