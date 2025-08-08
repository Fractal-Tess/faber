use serde::Deserialize;

use super::{ApiConfig, ContainerConfig, LoggingConfig};
use std::path::{Path, PathBuf};
use toml::de::Error as TomlDeError;

#[derive(Debug, thiserror::Error)]
pub enum FaberConfigError {
    #[error("Config file  was not found at: {0}")]
    ConfigNotFound(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(
        "Failed to parse TOML configuration: {}. Please check your config file for syntax errors.",
        extract_toml_error_message(_0)
    )]
    Toml(#[from] TomlDeError),
}

fn extract_toml_error_message(error: &TomlDeError) -> String {
    error.message().to_owned()
}

/// Main configuration structure loaded from default.toml
#[derive(Debug, Clone, Deserialize)]
pub struct FaberConfig {
    pub api: ApiConfig,
    pub container: ContainerConfig,
    pub logging: LoggingConfig,
}

impl FaberConfig {
    /// Load configuration from a specific file path
    pub fn load_from_path(path: &Path) -> Result<Self, FaberConfigError> {
        // 1. Load from specified config file path or default
        if !path.exists() {
            return Err(FaberConfigError::ConfigNotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)?;

        let config: FaberConfig = toml::from_str(&content)?;

        Ok(config)
    }
}
