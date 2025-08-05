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

impl GlobalConfig {
    /// Load configuration from a specific file path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        // 1. Load from specified config file path or default
        if !path.as_ref().exists() {
            return Err(faber_core::FaberError::Config(format!(
                "Config file not found: {}",
                path.as_ref().display()
            )));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            faber_core::FaberError::Config(format!("Failed to read config file: {e}"))
        })?;

        let config: GlobalConfig = toml::from_str(&content).map_err(|e| {
            faber_core::FaberError::Config(format!("Failed to parse config file: {e}"))
        })?;

        Ok(config)
    }

    pub fn apply_overrides(&mut self, overrides: ConfigOverrides) {
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

impl Display for GlobalConfig {
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
