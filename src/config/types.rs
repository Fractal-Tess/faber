use serde::Deserialize;

/// Main configuration structure loaded from default.toml
#[derive(Debug, Clone, Deserialize)]
pub struct FaberConfig {
    pub api: crate::api::ApiConfig,
    pub container: crate::container::ContainerConfig,
    pub queue: crate::queue::QueueConfig,
    pub logging: crate::logging::LoggingConfig,
}

/// Configuration overrides that can be applied to a FaberConfig
#[derive(Debug, Clone, Default)]
pub struct FaberConfigOverrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub auth_enabled: Option<bool>,
    pub workers: Option<usize>,
}

// Re-export all types for backward compatibility
pub use crate::api::*;
pub use crate::container::*;
pub use crate::filesystem::*;
pub use crate::queue::*;
pub use crate::security::*;
