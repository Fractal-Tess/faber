use std::path::Path;

use super::{FaberConfig, FaberConfigError};

impl FaberConfig {
    /// Load configuration from a specific file path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, FaberConfigError> {
        // 1. Load from specified config file path or default
        if !path.as_ref().exists() {
            return Err(FaberConfigError::ConfigNotFound(
                path.as_ref().to_path_buf(),
            ));
        }

        let content = std::fs::read_to_string(path)?;

        let config: FaberConfig = toml::from_str(&content)?;

        Ok(config)
    }
}
