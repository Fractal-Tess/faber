use crate::sandbox::container::{NamespaceSettings, ResourceLimits, SecurityLevel};
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::Display;
use std::fs;
use std::path::Path;

/// Main configuration structure loaded from config.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub version: String,
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
    pub mount_config_file: String,
    pub read_only_root: bool,
    pub allowed_extensions: Vec<String>,
    pub max_file_size: usize,
    pub max_files_per_request: usize,
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

impl Config {
    /// Load configuration from config.yaml file
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Load .env file in development, ignore errors in production
        dotenvy::dotenv().ok();

        // Try to load from config.yaml first
        let config_path = env::var("CONFIG_FILE").unwrap_or_else(|_| "config.yaml".to_string());

        if Path::new(&config_path).exists() {
            let config_content = fs::read_to_string(&config_path)?;
            let mut config: Config = serde_yaml::from_str(&config_content)?;

            // Override with environment variables if present
            config.override_from_env();

            Ok(config)
        } else {
            // Fallback to environment-only configuration
            Self::from_env()
        }
    }

    /// Load configuration from environment variables only (fallback)
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let open_mode = env::var("OPEN")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let api_key = if open_mode {
            "open-mode-no-auth".to_string()
        } else {
            env::var("API_KEY").unwrap_or_else(|_| "your-secret-api-key-here".to_string())
        };

        Ok(Config {
            server: ServerConfig {
                host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("PORT")
                    .unwrap_or_else(|_| "3000".to_string())
                    .parse()
                    .unwrap_or(3000),
                enable_swagger: env::var("ENABLE_SWAGGER")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .unwrap_or(true),
                enable_cors: false,
                request_timeout: 60,
                max_request_size: 10485760,
            },
            auth: AuthConfig {
                api_key,
                open_mode,
                token_expiration: 3600,
            },
            api: ApiConfig {
                health_endpoint: "/health".to_string(),
                execute_endpoint: "/execute-tasks".to_string(),
            },
            security: SecurityConfig {
                default_security_level: "standard".to_string(),
                seccomp: SeccompConfig {
                    enabled: true,
                    level: "basic".to_string(),
                    config_file: "seccomp.yaml".to_string(),
                },
                namespaces: NamespaceConfig {
                    pid: false,
                    mount: true,
                    network: true,
                    ipc: true,
                    uts: true,
                    user: true,
                    time: false,
                    cgroup: true,
                },
            },
            resource_limits: ResourceLimitsConfig {
                default: ResourceLimitSet {
                    memory_limit: 536870912,
                    cpu_time_limit: 10000000000,
                    wall_time_limit: 30000000000,
                    max_processes: 50,
                    max_fds: 256,
                    stack_limit: 4194304,
                    data_segment_limit: 268435456,
                    address_space_limit: 1073741824,
                    cpu_rate_limit: Some(50),
                    io_read_limit: Some(10485760),
                    io_write_limit: Some(10485760),
                },
                minimal: ResourceLimitSet {
                    memory_limit: 2147483648,
                    cpu_time_limit: 30000000000,
                    wall_time_limit: 60000000000,
                    max_processes: 100,
                    max_fds: 1024,
                    stack_limit: 8388608,
                    data_segment_limit: 1073741824,
                    address_space_limit: 4294967296,
                    cpu_rate_limit: None,
                    io_read_limit: None,
                    io_write_limit: None,
                },
                maximum: ResourceLimitSet {
                    memory_limit: 134217728,
                    cpu_time_limit: 5000000000,
                    wall_time_limit: 15000000000,
                    max_processes: 10,
                    max_fds: 64,
                    stack_limit: 1048576,
                    data_segment_limit: 67108864,
                    address_space_limit: 268435456,
                    cpu_rate_limit: Some(25),
                    io_read_limit: Some(1048576),
                    io_write_limit: Some(1048576),
                },
            },
            cgroups: CgroupsConfig {
                enabled: true,
                prefix: "faber".to_string(),
                version: "auto".to_string(),
                base_path: None,
                enable_cpu_rate_limit: true,
                enable_memory_limit: true,
                enable_process_limit: true,
            },
            container: ContainerConfig {
                work_dir_size_mb: 256,
                enable_mount_operations: true,
                uid: 1000,
                gid: 1000,
                hostname: "faber-container".to_string(),
                domain_name: "faber.local".to_string(),
            },
            filesystem: FilesystemConfig {
                mount_config_file: "mount.yaml".to_string(),
                read_only_root: false,
                allowed_extensions: vec![
                    ".cpp".to_string(),
                    ".c".to_string(),
                    ".py".to_string(),
                    ".js".to_string(),
                    ".java".to_string(),
                    ".rs".to_string(),
                    ".go".to_string(),
                    ".txt".to_string(),
                    ".md".to_string(),
                ],
                max_file_size: 1048576,
                max_files_per_request: 10,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                json_format: false,
                file_path: None,
                enable_request_logging: true,
                enable_performance_logging: true,
                enable_resource_logging: true,
            },
            validation: ValidationConfig {
                max_tasks_per_request: 100,
                max_command_length: 10000,
                max_env_value_length: 1000,
                max_file_content_length: 10485760,
                enable_dangerous_command_detection: true,
                blocked_commands: vec![
                    "rm -rf /".to_string(),
                    "dd if=/dev/zero".to_string(),
                    ":(){ :|:& };:".to_string(),
                    "mkfs".to_string(),
                    "fdisk".to_string(),
                    "parted".to_string(),
                ],
                blocked_env_vars: vec!["LD_PRELOAD".to_string(), "LD_LIBRARY_PATH".to_string()],
            },
            performance: PerformanceConfig {
                enable_connection_pooling: true,
                max_concurrent_connections: 1000,
                connection_timeout: 30,
                keep_alive_timeout: 5,
                enable_compression: true,
            },
            monitoring: MonitoringConfig {
                enable_metrics: true,
                metrics_port: 9090,
                enable_health_checks: true,
                health_check_interval: 30,
                enable_resource_monitoring: true,
            },
            development: DevelopmentConfig {
                debug: false,
                hot_reload: false,
                detailed_errors: false,
                enable_stack_traces: false,
            },
        })
    }

    /// Override configuration with environment variables
    fn override_from_env(&mut self) {
        // Server configuration overrides
        if let Ok(host) = env::var("HOST") {
            self.server.host = host;
        }
        if let Ok(port) = env::var("PORT") {
            if let Ok(port_num) = port.parse() {
                self.server.port = port_num;
            }
        }
        if let Ok(enable_swagger) = env::var("ENABLE_SWAGGER") {
            if let Ok(enabled) = enable_swagger.parse() {
                self.server.enable_swagger = enabled;
            }
        }

        // Auth configuration overrides
        if let Ok(open_mode) = env::var("OPEN") {
            if let Ok(open) = open_mode.parse() {
                self.auth.open_mode = open;
                if open {
                    self.auth.api_key = "open-mode-no-auth".to_string();
                }
            }
        }
        if let Ok(api_key) = env::var("API_KEY") {
            self.auth.api_key = api_key;
        }
    }

    /// Get security level from string
    pub fn get_security_level(&self) -> SecurityLevel {
        match self.security.default_security_level.as_str() {
            "minimal" => SecurityLevel::Minimal,
            "maximum" => SecurityLevel::Maximum,
            "custom" => SecurityLevel::Custom,
            _ => SecurityLevel::Standard,
        }
    }

    /// Get resource limits for a security level
    pub fn get_resource_limits(&self, level: SecurityLevel) -> ResourceLimits {
        let limit_set = match level {
            SecurityLevel::Minimal => &self.resource_limits.minimal,
            SecurityLevel::Maximum => &self.resource_limits.maximum,
            _ => &self.resource_limits.default,
        };

        ResourceLimits {
            memory_limit: limit_set.memory_limit,
            cpu_time_limit: limit_set.cpu_time_limit,
            wall_time_limit: limit_set.wall_time_limit,
            max_processes: limit_set.max_processes,
            max_fds: limit_set.max_fds,
            stack_limit: limit_set.stack_limit,
            data_segment_limit: limit_set.data_segment_limit,
            address_space_limit: limit_set.address_space_limit,
            cpu_rate_limit: limit_set.cpu_rate_limit,
            cpu_set_limit: None,
            io_read_limit: limit_set.io_read_limit,
            io_write_limit: limit_set.io_write_limit,
        }
    }

    /// Get namespace settings from configuration
    pub fn get_namespace_settings(&self) -> NamespaceSettings {
        NamespaceSettings {
            pid: self.security.namespaces.pid,
            mount: self.security.namespaces.mount,
            network: self.security.namespaces.network,
            ipc: self.security.namespaces.ipc,
            uts: self.security.namespaces.uts,
            user: self.security.namespaces.user,
            time: self.security.namespaces.time,
            cgroup: self.security.namespaces.cgroup,
        }
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{self:#?}")
    }
}
