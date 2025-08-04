use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    pub resource_limits: ResourceLimitsConfig,
    pub cgroups: CgroupsConfig,
    pub filesystem: crate::filesystem::FilesystemConfig,
    pub security: crate::security::SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupsConfig {
    pub enabled: bool,
    pub prefix: String,
    pub version: String,
    pub enable_cpu_rate_limit: bool,
    pub enable_memory_limit: bool,
    pub enable_process_limit: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            resource_limits: ResourceLimitsConfig::default(),
            cgroups: CgroupsConfig::default(),
            filesystem: crate::filesystem::FilesystemConfig::default(),
            security: crate::security::SecurityConfig::default(),
        }
    }
}

impl Default for ResourceLimitsConfig {
    fn default() -> Self {
        Self {
            memory_limit_kb: 524288,  // 512MB
            cpu_time_limit_ms: 10000, // 10 seconds
            max_cpu_cores: 1,
            wall_time_limit_ms: 30000, // 30 seconds
            max_processes: 50,
            max_fds: 256,
            stack_limit_kb: 4,            // 4MB
            data_segment_limit_kb: 256,   // 256MB
            address_space_limit_kb: 1024, // 1GB
            cpu_rate_limit_percent: 50,
            io_read_limit_kb_s: 10,  // 10MB/s
            io_write_limit_kb_s: 10, // 10MB/s
        }
    }
}

impl Default for CgroupsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefix: "faber".to_string(),
            version: "v2".to_string(),
            enable_cpu_rate_limit: true,
            enable_memory_limit: true,
            enable_process_limit: true,
        }
    }
}
