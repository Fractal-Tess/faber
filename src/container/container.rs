use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, error, info, warn};

use super::filesystem::ContainerFilesystem;
use crate::config::FaberConfig;

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("Failed to create container directory: {0}")]
    DirectoryCreation(#[from] std::io::Error),
    #[error("Failed to mount filesystem: {0}")]
    MountError(String),
    #[error("Failed to cleanup container: {0}")]
    CleanupError(String),
    #[error("Container not initialized")]
    NotInitialized,
    #[error("Filesystem error: {0}")]
    Filesystem(#[from] super::filesystem::FilesystemError),
}

/// Container instance that manages filesystem isolation
pub struct Container {
    config: Arc<FaberConfig>,
    container_id: String,
    container_root: PathBuf,
    filesystem: Option<ContainerFilesystem>,
    initialized: bool,
}

impl Container {
    pub fn new(config: Arc<FaberConfig>) -> Self {
        let container_id = uuid::Uuid::new_v4().to_string();
        let container_path = PathBuf::from(&config.container.base_dir).join(&container_id);

        Self {
            config,
            container_id,
            container_root: container_path,
            filesystem: None,
            initialized: false,
        }
    }

    /// Initialize the container filesystem
    pub async fn initialize(&mut self) -> Result<(), ContainerError> {
        info!("Initializing container at {:?}", self.container_root);

        // Initialize filesystem
        let mut filesystem = ContainerFilesystem::new(
            self.container_root.clone(),
            self.config.container.filesystem.clone(),
        );
        filesystem.initialize().await?;
        self.filesystem = Some(filesystem);

        self.initialized = true;
        info!("Container {} initialized successfully", self.container_id);
        Ok(())
    }

    /// Cleanup and destroy the container
    pub async fn cleanup(&mut self) -> Result<(), ContainerError> {
        if !self.initialized {
            warn!(
                "Container {} not initialized, skipping cleanup",
                self.container_id
            );
            return Ok(());
        }

        info!("Cleaning up container {}", self.container_id);

        // Cleanup filesystem
        if let Some(filesystem) = &mut self.filesystem {
            filesystem.cleanup().await?;
        }

        self.initialized = false;
        info!("Container {} cleanup completed", self.container_id);
        Ok(())
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        if self.initialized {
            warn!("Container {} dropped without cleanup", self.container_id);
            // Note: We can't do async cleanup in Drop, so we just log a warning
        }
    }
}
