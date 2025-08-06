use std::path::Path;
use tracing::{debug, info};

use super::{FaberConfig, FaberConfigError};

impl FaberConfig {
    /// Load configuration from a specific file path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, FaberConfigError> {
        debug!("Loading configuration from path: {:?}", path.as_ref());

        // 1. Load from specified config file path or default
        if !path.as_ref().exists() {
            debug!("Configuration file does not exist: {:?}", path.as_ref());
            return Err(FaberConfigError::ConfigNotFound(
                path.as_ref().to_path_buf(),
            ));
        }

        debug!("Configuration file exists, reading content...");
        let content = std::fs::read_to_string(path)?;
        debug!("Configuration file content length: {} bytes", content.len());

        debug!("Parsing TOML configuration...");
        let config: FaberConfig = toml::from_str(&content)?;

        debug!("Configuration loaded successfully");
        debug!("Container config: {:?}", config.container);
        debug!("Filesystem config: {:?}", config.container.filesystem);

        Ok(config)
    }
}
