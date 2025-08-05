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

// Simple error type for config operations
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

impl FaberConfig {
    /// Load configuration from a specific file path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        // 1. Load from specified config file path or default
        if !path.as_ref().exists() {
            return Err(ConfigError::Config(format!(
                "Config file not found: {}",
                path.as_ref().display()
            )));
        }

        let content = fs::read_to_string(path)?;

        let config: FaberConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::Config(format!("Failed to parse config file: {e}")))?;

        Ok(config)
    }

    pub fn apply_overrides(&mut self, overrides: FaberConfigOverrides) {
        if let Some(host) = overrides.host {
            self.api.host = host;
        }
        if let Some(port) = overrides.port {
            self.api.port = port;
        }
        if let Some(auth_enabled) = overrides.auth_enabled {
            self.api.auth.enable = auth_enabled;
        }
        if let Some(workers) = overrides.workers {
            self.queue.worker_count = workers;
        }
    }
}

impl Display for FaberConfig {
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
