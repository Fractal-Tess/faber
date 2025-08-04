use serde::{Deserialize, Serialize};

/// Main configuration structure loaded from config.yaml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub api: ApiConfig,
    pub security: SecurityConfig,
    pub resource_limits: ResourceLimitsConfig,
    pub cgroups: CgroupsConfig,
    pub container: ContainerConfig,
    pub filesystem: FilesystemConfig,
    pub logging: LoggingConfig,
    pub validation: ValidationConfig,
    pub performance: PerformanceConfig,
    pub monitoring: MonitoringConfig,
    pub development: DevelopmentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub enable_swagger: bool,
    pub enable_cors: bool,
    pub request_timeout: u64,
    pub max_request_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
    pub open_mode: bool,
    pub token_expiration: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub health_endpoint: String,
    pub execute_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub default_security_level: String,
    pub seccomp: SeccompConfig,
    pub namespaces: NamespaceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompConfig {
    pub enabled: bool,
    pub level: String,
    pub config_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceConfig {
    pub pid: bool,
    pub mount: bool,
    pub network: bool,
    pub ipc: bool,
    pub uts: bool,
    pub user: bool,
    pub time: bool,
    pub cgroup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitsConfig {
    pub default: ResourceLimitSet,
    pub minimal: ResourceLimitSet,
    pub maximum: ResourceLimitSet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitSet {
    pub memory_limit: u64,
    pub cpu_time_limit: u64,
    pub wall_time_limit: u64,
    pub max_processes: u32,
    pub max_fds: u64,
    pub stack_limit: u64,
    pub data_segment_limit: u64,
    pub address_space_limit: u64,
    pub cpu_rate_limit: Option<u32>,
    pub io_read_limit: Option<u64>,
    pub io_write_limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupsConfig {
    pub enabled: bool,
    pub prefix: String,
    pub base_path: Option<String>,
    pub enable_cpu_rate_limit: bool,
    pub enable_memory_limit: bool,
    pub enable_process_limit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub work_dir_size_mb: u32,
    pub enable_mount_operations: bool,
    pub uid: u32,
    pub gid: u32,
    pub hostname: String,
    pub domain_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemConfig {
    pub read_only_root: bool,
    pub allowed_extensions: Vec<String>,
    pub max_file_size: usize,
    pub max_files_per_request: usize,
    pub mounts: MountConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    pub work_dir: String,
    pub tmp_dir: String,
    pub read_only_paths: Vec<String>,
    pub writable_paths: Vec<String>,
    pub tmpfs_size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub json_format: bool,
    pub file_path: Option<String>,
    pub enable_request_logging: bool,
    pub enable_performance_logging: bool,
    pub enable_resource_logging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub max_tasks_per_request: usize,
    pub max_command_length: usize,
    pub max_env_value_length: usize,
    pub max_file_content_length: usize,
    pub enable_dangerous_command_detection: bool,
    pub blocked_commands: Vec<String>,
    pub blocked_env_vars: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub enable_connection_pooling: bool,
    pub max_concurrent_connections: usize,
    pub connection_timeout: u64,
    pub keep_alive_timeout: u64,
    pub enable_compression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_metrics: bool,
    pub metrics_port: u16,
    pub enable_health_checks: bool,
    pub health_check_interval: u64,
    pub enable_resource_monitoring: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopmentConfig {
    pub debug: bool,
    pub hot_reload: bool,
    pub detailed_errors: bool,
    pub enable_stack_traces: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            enable_swagger: true,
            enable_cors: true,
            request_timeout: 30,
            max_request_size: 1024 * 1024, // 1MB
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            api_key: "default-api-key".to_string(),
            open_mode: false,
            token_expiration: 3600,
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            health_endpoint: "/health".to_string(),
            execute_endpoint: "/execute".to_string(),
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            default_security_level: "medium".to_string(),
            seccomp: SeccompConfig::default(),
            namespaces: NamespaceConfig::default(),
        }
    }
}

impl Default for SeccompConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: "medium".to_string(),
            config_file: "seccomp.yaml".to_string(),
        }
    }
}

impl Default for NamespaceConfig {
    fn default() -> Self {
        Self {
            pid: true,
            mount: true,
            network: true,
            ipc: true,
            uts: true,
            user: true,
            time: true,
            cgroup: true,
        }
    }
}

impl Default for ResourceLimitsConfig {
    fn default() -> Self {
        Self {
            default: ResourceLimitSet::default(),
            minimal: ResourceLimitSet::minimal(),
            maximum: ResourceLimitSet::maximum(),
        }
    }
}

impl Default for ResourceLimitSet {
    fn default() -> Self {
        Self {
            memory_limit: 512 * 1024 * 1024,     // 512MB
            cpu_time_limit: 30 * 1_000_000_000,  // 30 seconds
            wall_time_limit: 60 * 1_000_000_000, // 60 seconds
            max_processes: 10,
            max_fds: 100,
            stack_limit: 8 * 1024 * 1024,                // 8MB
            data_segment_limit: 256 * 1024 * 1024,       // 256MB
            address_space_limit: 1 * 1024 * 1024 * 1024, // 1GB
            cpu_rate_limit: Some(100),
            io_read_limit: Some(100 * 1024 * 1024),  // 100MB
            io_write_limit: Some(100 * 1024 * 1024), // 100MB
        }
    }
}

impl ResourceLimitSet {
    pub fn minimal() -> Self {
        Self {
            memory_limit: 64 * 1024 * 1024,      // 64MB
            cpu_time_limit: 5 * 1_000_000_000,   // 5 seconds
            wall_time_limit: 10 * 1_000_000_000, // 10 seconds
            max_processes: 5,
            max_fds: 50,
            stack_limit: 1 * 1024 * 1024,           // 1MB
            data_segment_limit: 32 * 1024 * 1024,   // 32MB
            address_space_limit: 128 * 1024 * 1024, // 128MB
            cpu_rate_limit: Some(50),
            io_read_limit: Some(10 * 1024 * 1024),  // 10MB
            io_write_limit: Some(10 * 1024 * 1024), // 10MB
        }
    }

    pub fn maximum() -> Self {
        Self {
            memory_limit: 2 * 1024 * 1024 * 1024, // 2GB
            cpu_time_limit: 300 * 1_000_000_000,  // 5 minutes
            wall_time_limit: 600 * 1_000_000_000, // 10 minutes
            max_processes: 50,
            max_fds: 1000,
            stack_limit: 32 * 1024 * 1024,               // 32MB
            data_segment_limit: 1 * 1024 * 1024 * 1024,  // 1GB
            address_space_limit: 4 * 1024 * 1024 * 1024, // 4GB
            cpu_rate_limit: Some(200),
            io_read_limit: Some(1 * 1024 * 1024 * 1024), // 1GB
            io_write_limit: Some(1 * 1024 * 1024 * 1024), // 1GB
        }
    }
}

impl Default for CgroupsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            prefix: "faber".to_string(),
            base_path: None,
            enable_cpu_rate_limit: true,
            enable_memory_limit: true,
            enable_process_limit: true,
        }
    }
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            work_dir_size_mb: 100,
            enable_mount_operations: false,
            uid: 1000,
            gid: 1000,
            hostname: "faber-container".to_string(),
            domain_name: "faber.local".to_string(),
        }
    }
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            read_only_root: true,
            allowed_extensions: vec!["txt".to_string(), "py".to_string(), "js".to_string()],
            max_file_size: 1024 * 1024, // 1MB
            max_files_per_request: 10,
            mounts: MountConfig::default(),
        }
    }
}

impl Default for MountConfig {
    fn default() -> Self {
        Self {
            work_dir: "/work".to_string(),
            tmp_dir: "/tmp".to_string(),
            read_only_paths: vec!["/etc".to_string(), "/usr/bin".to_string()],
            writable_paths: vec!["/tmp".to_string()],
            tmpfs_size: "100M".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            file_path: None,
            enable_request_logging: true,
            enable_performance_logging: false,
            enable_resource_logging: true,
        }
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_tasks_per_request: 10,
            max_command_length: 1024,
            max_env_value_length: 4096,
            max_file_content_length: 1024 * 1024, // 1MB
            enable_dangerous_command_detection: true,
            blocked_commands: vec!["rm".to_string(), "dd".to_string(), "mkfs".to_string()],
            blocked_env_vars: vec!["PATH".to_string(), "LD_LIBRARY_PATH".to_string()],
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            enable_connection_pooling: true,
            max_concurrent_connections: 1000,
            connection_timeout: 30,
            keep_alive_timeout: 60,
            enable_compression: true,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enable_metrics: false,
            metrics_port: 9090,
            enable_health_checks: true,
            health_check_interval: 30,
            enable_resource_monitoring: true,
        }
    }
}

impl Default for DevelopmentConfig {
    fn default() -> Self {
        Self {
            debug: false,
            hot_reload: false,
            detailed_errors: false,
            enable_stack_traces: false,
        }
    }
}
