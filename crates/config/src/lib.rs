use faber_core::Result;
use std::env;
use std::fmt::Display;
use std::fs;
use std::path::Path;

pub mod api;
pub mod filesystem;
pub mod queue;
pub mod sandbox;
pub mod security;
pub mod types;

pub use types::*;

impl Config {
    /// Load configuration with CLI overrides
    pub fn load(
        config_path: Option<String>,
        host: Option<String>,
        port: Option<u16>,
        open_mode: bool,
        workers: Option<usize>,
    ) -> Result<Self> {
        // 1. Load from config file (or default)
        let config_path = config_path.unwrap_or("config/default.toml".to_string());
        let mut config = Self::from_file(config_path)?;

        // 2. Override with environment variables
        config.override_from_env();

        // 3. Override with CLI options (highest priority)
        if open_mode {
            config.api.auth.enable = "true".to_string();
        }
        if let Some(host) = host {
            config.api.host = host;
        }
        if let Some(port) = port {
            config.api.port = port;
        }
        if let Some(workers) = workers {
            config.queue.worker_count = workers;
            config.queue.max_concurrent_sandboxes = workers;
        }

        Ok(config)
    }

    /// Load configuration from a specific file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(|e| {
            faber_core::FaberError::Config(format!("Failed to read config file: {e}"))
        })?;

        let mut config: Config = toml::from_str(&content).map_err(|e| {
            faber_core::FaberError::Config(format!("Failed to parse config file: {e}"))
        })?;

        config.override_from_env();
        Ok(config)
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let mut config = Config::default();
        config.override_from_env();
        Ok(config)
    }

    /// Parse environment variable value with fallback
    fn parse_env_value(value: &str) -> String {
        if value.starts_with("env:") {
            let parts: Vec<&str> = value.splitn(2, '|').collect();
            let env_var = parts[0].strip_prefix("env:").unwrap_or("");

            if let Ok(env_value) = env::var(env_var) {
                return env_value;
            }

            // Return fallback value if provided
            if parts.len() > 1 {
                return parts[1].to_string();
            }
        }
        value.to_string()
    }

    /// Parse boolean environment variable value
    fn parse_env_bool(value: &str) -> bool {
        let parsed = Self::parse_env_value(value);
        parsed.parse().unwrap_or(false)
    }

    /// Parse integer environment variable value
    fn parse_env_u32(value: &str) -> u32 {
        let parsed = Self::parse_env_value(value);
        parsed.parse().unwrap_or(0)
    }

    /// Override configuration values from environment variables
    fn override_from_env(&mut self) {
        // API configuration
        if let Ok(host) = env::var("FABER_API_HOST") {
            self.api.host = host;
        }
        if let Ok(port) = env::var("FABER_API_PORT") {
            if let Ok(port_num) = port.parse() {
                self.api.port = port_num;
            }
        }

        // CORS configuration
        if let Ok(enable_cors) = env::var("FABER_API_CORS_ENABLE") {
            self.api.cors.enable_cors = enable_cors.parse().unwrap_or(false);
        }
        if let Ok(origins) = env::var("FABER_API_CORS_ALLOWED_ORIGINS") {
            self.api.cors.cors_allowed_origins = origins;
        }
        if let Ok(methods) = env::var("FABER_API_CORS_ALLOWED_METHODS") {
            self.api.cors.cors_allowed_methods = methods;
        }
        if let Ok(headers) = env::var("FABER_API_CORS_ALLOWED_HEADERS") {
            self.api.cors.cors_allowed_headers = headers;
        }
        if let Ok(credentials) = env::var("FABER_API_CORS_ALLOW_CREDENTIALS") {
            self.api.cors.cors_allow_credentials = credentials.parse().unwrap_or(false);
        }

        // Request configuration
        if let Ok(size) = env::var("API_MAX_REQUEST_SIZE_KB") {
            if let Ok(size_num) = size.parse() {
                self.api.request.max_request_size_kb = size_num;
            }
        }

        // Auth configuration
        self.api.auth.enable = Self::parse_env_bool(&self.api.auth.enable).to_string();
        self.api.auth.secret_key = Self::parse_env_value(&self.api.auth.secret_key);

        // Sandbox resource limits
        if let Ok(memory) = env::var("FABER_SANDBOX_MEMORY_LIMIT_KB") {
            if let Ok(memory_num) = memory.parse() {
                self.sandbox.resource_limits.memory_limit_kb = memory_num;
            }
        }
        if let Ok(cpu_time) = env::var("FABER_SANDBOX_CPU_TIME_LIMIT_MS") {
            if let Ok(cpu_time_num) = cpu_time.parse() {
                self.sandbox.resource_limits.cpu_time_limit_ms = cpu_time_num;
            }
        }
        if let Ok(cpu_cores) = env::var("FABER_SANDBOX_MAX_CPU_CORES") {
            if let Ok(cpu_cores_num) = cpu_cores.parse() {
                self.sandbox.resource_limits.max_cpu_cores = cpu_cores_num;
            }
        }
        if let Ok(wall_time) = env::var("FABER_SANDBOX_WALL_TIME_LIMIT_MS") {
            if let Ok(wall_time_num) = wall_time.parse() {
                self.sandbox.resource_limits.wall_time_limit_ms = wall_time_num;
            }
        }
        if let Ok(processes) = env::var("FABER_SANDBOX_MAX_PROCESSES") {
            if let Ok(processes_num) = processes.parse() {
                self.sandbox.resource_limits.max_processes = processes_num;
            }
        }
        if let Ok(fds) = env::var("FABER_SANDBOX_MAX_FDS") {
            if let Ok(fds_num) = fds.parse() {
                self.sandbox.resource_limits.max_fds = fds_num;
            }
        }
        if let Ok(stack) = env::var("FABER_SANDBOX_STACK_LIMIT_KB") {
            if let Ok(stack_num) = stack.parse() {
                self.sandbox.resource_limits.stack_limit_kb = stack_num;
            }
        }
        if let Ok(data_seg) = env::var("FABER_SANDBOX_DATA_SEGMENT_LIMIT_KB") {
            if let Ok(data_seg_num) = data_seg.parse() {
                self.sandbox.resource_limits.data_segment_limit_kb = data_seg_num;
            }
        }
        if let Ok(addr_space) = env::var("FABER_SANDBOX_ADDRESS_SPACE_LIMIT_KB") {
            if let Ok(addr_space_num) = addr_space.parse() {
                self.sandbox.resource_limits.address_space_limit_kb = addr_space_num;
            }
        }
        if let Ok(cpu_rate) = env::var("FABER_SANDBOX_CPU_RATE_LIMIT_PERCENT") {
            if let Ok(cpu_rate_num) = cpu_rate.parse() {
                self.sandbox.resource_limits.cpu_rate_limit_percent = cpu_rate_num;
            }
        }
        if let Ok(io_read) = env::var("FABER_SANDBOX_IO_READ_LIMIT_KB_S") {
            if let Ok(io_read_num) = io_read.parse() {
                self.sandbox.resource_limits.io_read_limit_kb_s = io_read_num;
            }
        }
        if let Ok(io_write) = env::var("FABER_SANDBOX_IO_WRITE_LIMIT_KB_S") {
            if let Ok(io_write_num) = io_write.parse() {
                self.sandbox.resource_limits.io_write_limit_kb_s = io_write_num;
            }
        }

        // Cgroups configuration
        if let Ok(enabled) = env::var("FABER_SANDBOX_CGROUPS_ENABLED") {
            self.sandbox.cgroups.enabled = enabled.parse().unwrap_or(true);
        }
        if let Ok(cpu_rate) = env::var("FABER_SANDBOX_ENABLE_CPU_RATE_LIMIT") {
            self.sandbox.cgroups.enable_cpu_rate_limit = cpu_rate.parse().unwrap_or(true);
        }
        if let Ok(memory) = env::var("FABER_SANDBOX_ENABLE_MEMORY_LIMIT") {
            self.sandbox.cgroups.enable_memory_limit = memory.parse().unwrap_or(true);
        }
        if let Ok(process) = env::var("FABER_SANDBOX_ENABLE_PROCESS_LIMIT") {
            self.sandbox.cgroups.enable_process_limit = process.parse().unwrap_or(true);
        }

        // Namespace configuration
        if let Ok(pid) = env::var("FABER_SANDBOX_PID_NAMESPACE_ENABLED") {
            self.sandbox.security.namespaces.pid = pid.parse().unwrap_or(false);
        }
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Faber Configuration:")?;
        writeln!(f, "  API: {}:{}", self.api.host, self.api.port)?;
        writeln!(f, "  CORS: enabled={}", self.api.cors.enable_cors)?;
        writeln!(f, "  Auth: enabled={}", self.api.auth.enable)?;
        writeln!(
            f,
            "  Sandbox Security: level={}",
            self.sandbox.security.default_security_level
        )?;
        writeln!(
            f,
            "  Resource Limits: memory={}KB, cpu_time={}ms",
            self.sandbox.resource_limits.memory_limit_kb,
            self.sandbox.resource_limits.cpu_time_limit_ms
        )?;
        Ok(())
    }
}
