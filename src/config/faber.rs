use serde::Deserialize;

use super::{ApiConfig, ContainerConfig, ExecutorConfig, LoggingConfig};
use std::path::PathBuf;

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
    Toml(#[from] toml::de::Error),
}

fn extract_toml_error_message(error: &toml::de::Error) -> String {
    error.message().to_owned()
}

/// Main configuration structure loaded from default.toml
#[derive(Debug, Clone, Deserialize)]
pub struct FaberConfig {
    pub api: ApiConfig,
    pub container: ContainerConfig,
    pub executor: ExecutorConfig,
    pub logging: LoggingConfig,
}

/// Configuration overrides that can be applied to a FaberConfig
#[derive(Debug, Clone, Default)]
pub struct FaberConfigOverrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub auth_enabled: Option<bool>,
    pub workers: Option<u16>,
}
