use serde::Deserialize;

/// Main configuration structure loaded from default.toml
#[derive(Debug, Clone, Deserialize)]
pub struct GlobalConfig {
    pub api: crate::api::ApiConfig,
    pub sandbox: crate::sandbox::SandboxConfig,
    pub queue: crate::queue::QueueConfig,
}

// TODO: Improve this type
/// Configuration overrides that can be applied to a GlobalConfig
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub auth_enabled: Option<bool>,
    pub workers: Option<u16>,
}

// Re-export all types for backward compatibility
pub use crate::api::*;
pub use crate::filesystem::*;
pub use crate::queue::*;
pub use crate::sandbox::*;
pub use crate::security::*;
