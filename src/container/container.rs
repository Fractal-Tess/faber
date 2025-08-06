use nix::libc::{gid_t, uid_t};
use nix::mount::{MsFlags, mount};
use nix::sched::{CloneFlags, unshare};
use nix::unistd::{setgid, setuid};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, error, info, warn};

use super::filesystem::ContainerFilesystem;
use super::namespaces::ContainerNamespaces;
use crate::config::FaberConfig;
use crate::config::NamespaceConfig;

#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    #[error("Failed to create container directory: {0}")]
    DirectoryCreation(#[from] std::io::Error),
    #[error("Failed to mount filesystem: {0}")]
    MountError(String),
    #[error("Failed to create namespaces: {0}")]
    NamespaceError(String),
    #[error("Failed to cleanup container: {0}")]
    CleanupError(String),
    #[error("Container not initialized")]
    NotInitialized,
    #[error("Filesystem error: {0}")]
    Filesystem(#[from] super::filesystem::FilesystemError),
    #[error("Namespace error: {0}")]
    Namespace(#[from] super::namespaces::NamespaceError),
}

/// Container instance that manages isolation and filesystem
pub struct Container {
    config: Arc<FaberConfig>,
    container_id: String,
    container_path: PathBuf,
    filesystem: Option<ContainerFilesystem>,
    namespaces: Option<ContainerNamespaces>,
    initialized: bool,
}

impl Container {
    pub fn new(config: Arc<FaberConfig>) -> Self {
        //TODO: Maybe change the id to the worker id instead?
        // This will be an issue if there is problem unmounting stuff and another worker is trying to use the same container
        // FOr now, we will use a random uuid for the container id
        let container_id = uuid::Uuid::new_v4().to_string();
        let container_path = PathBuf::from(&config.container.base_dir).join(&container_id);

        Self {
            config,
            container_id,
            container_path,
            filesystem: None,
            namespaces: None,
            initialized: false,
        }
    }

    /// Initialize the container with filesystem and namespaces
    pub async fn initialize(&mut self) -> Result<(), ContainerError> {
        info!("Initializing container at {:?}", self.container_path);
        debug!("Container ID: {}", self.container_id);
        debug!("Container config: {:?}", self.config.container);

        // Create container directory
        debug!("Creating container directory...");
        self.create_container_directory().await?;
        debug!("Container directory created successfully");

        // Initialize filesystem
        debug!("Initializing filesystem...");
        debug!("Container path: {:?}", self.container_path);
        debug!("Filesystem config: {:?}", self.config.container.filesystem);

        let mut filesystem = ContainerFilesystem::new(
            self.container_path.clone(),
            self.config.container.filesystem.clone(),
        );
        debug!("ContainerFilesystem created, initializing...");
        filesystem.initialize().await?;
        debug!("Filesystem initialized successfully");
        self.filesystem = Some(filesystem);

        // Initialize namespaces
        debug!("Initializing namespaces...");
        let mut namespaces =
            ContainerNamespaces::new(self.config.container.security.namespaces.clone());
        namespaces.initialize().await?;
        debug!("Namespaces initialized successfully");
        self.namespaces = Some(namespaces);

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
            if let Err(e) = filesystem.cleanup().await {
                error!("Failed to cleanup filesystem: {}", e);
            }
        }

        // Cleanup namespaces
        if let Some(namespaces) = &mut self.namespaces {
            if let Err(e) = namespaces.cleanup().await {
                error!("Failed to cleanup namespaces: {}", e);
            }
        }

        // Remove container directory
        if let Err(e) = self.remove_container_directory().await {
            error!("Failed to remove container directory: {}", e);
        }

        self.initialized = false;
        info!("Container {} cleanup completed", self.container_id);
        Ok(())
    }

    /// Get the container ID
    pub fn id(&self) -> &str {
        &self.container_id
    }

    /// Get the container path
    pub fn path(&self) -> &PathBuf {
        &self.container_path
    }

    /// Check if container is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the container root path for command execution
    pub fn get_root_path(&self) -> Result<&PathBuf, ContainerError> {
        if !self.initialized {
            return Err(ContainerError::NotInitialized);
        }
        Ok(&self.container_path)
    }

    /// Get namespace flags for unshare command
    pub fn get_namespace_flags(&self) -> Result<Vec<&'static str>, ContainerError> {
        if !self.initialized {
            return Err(ContainerError::NotInitialized);
        }

        if let Some(namespaces) = &self.namespaces {
            Ok(namespaces.get_unshare_flags())
        } else {
            Err(ContainerError::NotInitialized)
        }
    }

    /// Get namespace configuration for debugging and validation
    pub fn get_namespace_config(&self) -> Result<&NamespaceConfig, ContainerError> {
        if !self.initialized {
            return Err(ContainerError::NotInitialized);
        }

        if let Some(namespaces) = &self.namespaces {
            Ok(namespaces.get_config())
        } else {
            Err(ContainerError::NotInitialized)
        }
    }

    /// Get enabled namespaces for debugging
    pub fn get_enabled_namespaces(&self) -> Result<Vec<&'static str>, ContainerError> {
        if !self.initialized {
            return Err(ContainerError::NotInitialized);
        }

        if let Some(namespaces) = &self.namespaces {
            Ok(namespaces.get_enabled_namespaces())
        } else {
            Err(ContainerError::NotInitialized)
        }
    }

    /// Get nix CloneFlags for namespace creation
    pub fn get_clone_flags(&self) -> Result<CloneFlags, ContainerError> {
        if !self.initialized {
            return Err(ContainerError::NotInitialized);
        }

        if let Some(namespaces) = &self.namespaces {
            Ok(namespaces.get_clone_flags())
        } else {
            Err(ContainerError::NotInitialized)
        }
    }

    /// Create the container directory structure
    async fn create_container_directory(&self) -> Result<(), ContainerError> {
        info!("Creating container directory: {:?}", self.container_path);
        debug!("Container path exists: {}", self.container_path.exists());
        debug!(
            "Container path is directory: {}",
            self.container_path.is_dir()
        );

        // Create main container directory
        debug!("Creating main container directory...");
        fs::create_dir_all(&self.container_path).await?;
        debug!("Main container directory created successfully");

        // Create essential directories
        let essential_dirs = [
            self.config.container.work_dir.as_str(),
            self.config.container.tmp_dir.as_str(),
            "proc",
            "sys",
            "dev",
        ];
        debug!("Creating essential directories: {:?}", essential_dirs);
        for dir in &essential_dirs {
            let dir_path = self.container_path.join(dir);
            debug!("Creating directory: {:?}", dir_path);
            fs::create_dir_all(&dir_path).await?;
            debug!("Directory created successfully: {:?}", dir_path);
        }

        debug!("All container directories created successfully");
        Ok(())
    }

    /// Remove the container directory
    async fn remove_container_directory(&self) -> Result<(), ContainerError> {
        debug!("Removing container directory: {:?}", self.container_path);

        if self.container_path.exists() {
            fs::remove_dir_all(&self.container_path).await?;
        }

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
