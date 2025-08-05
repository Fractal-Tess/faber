use std::fmt::Display;
use std::fs;
use std::path::Path;

pub mod api;
pub mod container;
mod error;
pub mod filesystem;
pub mod logging;
pub mod queue;
pub mod security;
pub mod types;

pub use types::*;

use crate::error::FaberConfigError;

impl FaberConfig {
    /// Load configuration from a specific file path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, FaberConfigError> {
        // 1. Load from specified config file path or default
        if !path.as_ref().exists() {
            return Err(FaberConfigError::ConfigNotFound(
                path.as_ref().to_path_buf(),
            ));
        }

        let content = fs::read_to_string(path)?;

        let config: FaberConfig = toml::from_str(&content)?;

        Ok(config)
    }
}

impl Display for FaberConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Faber Configuration:")?;
        writeln!(f, "  API: {}:{}", self.api.host, self.api.port)?;
        writeln!(f, "  CORS: enabled={}", self.api.cors.enable_cors)?;
        writeln!(f, "  Auth: enabled={}", self.api.auth.enable)?;
        Ok(())
    }
}
