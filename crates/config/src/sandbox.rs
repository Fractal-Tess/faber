use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SandboxConfig {
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
    pub version: String,
    pub enable_cpu_rate_limit: bool,
    pub enable_memory_limit: bool,
    pub enable_process_limit: bool,
}
