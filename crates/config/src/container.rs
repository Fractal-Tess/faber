use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize)]
pub struct ContainerConfig {
    pub resource_limits: ResourceLimitsConfig,
    pub cgroups: CgroupsConfig,
    pub filesystem: crate::filesystem::FilesystemConfig,
    pub security: crate::security::SecurityConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceLimitsConfig {
    pub memory_limit_kb: u32,
    pub cpu_time_limit_ms: u32,
    pub max_cpu_cores: u32,
    pub wall_time_limit_ms: u32,
    pub max_processes: u32,
    pub max_fds: u32,
    pub stack_limit_kb: u32,
    pub data_segment_limit_kb: u32,
    pub address_space_limit_kb: u32,
    pub cpu_rate_limit_percent: u32,
    pub io_read_limit_kb_s: u32,
    pub io_write_limit_kb_s: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CgroupsConfig {
    pub enabled: bool,
    pub prefix: String,
    pub version: CgroupVersion,
    pub enable_cpu_rate_limit: bool,
    pub enable_memory_limit: bool,
    pub enable_process_limit: bool,
}

#[derive(Debug, Clone)]
pub enum CgroupVersion {
    V1,
    V2,
}

impl<'de> Deserialize<'de> for CgroupVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = u8::deserialize(deserializer)?;
        match s {
            1 => Err(serde::de::Error::custom(
                "Cgroup version 1 is not supported",
            )),
            2 => Ok(CgroupVersion::V2),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid cgroup version: {s}"
            ))),
        }
    }
}
