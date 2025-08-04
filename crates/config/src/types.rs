use serde::{Deserialize, Serialize};

/// Main configuration structure loaded from default.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub api: crate::api::ApiConfig,
    pub sandbox: crate::sandbox::SandboxConfig,
    pub queue: crate::queue::QueueConfig,
}

// Re-export all types for backward compatibility
pub use crate::api::*;
pub use crate::filesystem::*;
pub use crate::queue::*;
pub use crate::sandbox::*;
pub use crate::security::*;
