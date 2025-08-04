use faber_core::Result;
use std::env;
use std::fmt::Display;
use std::fs;
use std::path::Path;

pub mod types;

pub use types::*;

impl Config {
    /// Load configuration from config.yaml or environment variables
    pub fn load() -> Result<Self> {
        // Load .env file if it exists
        dotenvy::dotenv().ok();

        // Try to load from config file first
        if let Ok(config) = Self::from_file("config/config.yaml") {
            return Ok(config);
        }

        // Fall back to environment variables
        Self::from_env()
    }

    /// Load configuration from a specific file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path).map_err(|e| {
            faber_core::FaberError::Config(format!("Failed to read config file: {e}"))
        })?;

        let mut config: Config = serde_yaml::from_str(&content).map_err(|e| {
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

    /// Override configuration values from environment variables
    fn override_from_env(&mut self) {
        if let Ok(host) = env::var("FABER_HOST") {
            self.server.host = host;
        }
        if let Ok(port) = env::var("FABER_PORT") {
            if let Ok(port_num) = port.parse() {
                self.server.port = port_num;
            }
        }
        if let Ok(api_key) = env::var("FABER_API_KEY") {
            self.auth.api_key = api_key;
        }
        if let Ok(open_mode) = env::var("FABER_OPEN_MODE") {
            self.auth.open_mode = open_mode.parse().unwrap_or(false);
        }
        if let Ok(level) = env::var("FABER_LOG_LEVEL") {
            self.logging.level = level;
        }
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Faber Configuration:")?;
        writeln!(f, "  Server: {}:{}", self.server.host, self.server.port)?;
        writeln!(f, "  Auth: open_mode={}", self.auth.open_mode)?;
        writeln!(f, "  Logging: level={}", self.logging.level)?;
        writeln!(
            f,
            "  Security: level={}",
            self.security.default_security_level
        )?;
        Ok(())
    }
}
