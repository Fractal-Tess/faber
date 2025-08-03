//! Main container sandbox implementation
//!
//! This module provides the primary ContainerSandbox struct that orchestrates
//! container creation, execution, and cleanup.

use crate::sandbox::{
    MountConfig, NamespaceConfig, Result, SandboxError,
    namespaces::NamespaceManager,
    resource_limits::{ResourceLimits, ResourceUsage},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Instant;
use tempfile::TempDir;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Configuration for a container sandbox
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerConfig {
    /// Namespace configuration
    pub namespaces: NamespaceConfig,
    /// Resource limits
    pub limits: ResourceLimits,
    /// Mount configuration
    pub mounts: Option<MountConfig>,
    /// Working directory inside container
    pub work_dir: PathBuf,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether to enable networking
    pub networking: bool,
}

impl Default for ContainerConfig {
    fn default() -> Self {
        Self {
            namespaces: NamespaceConfig::default(),
            limits: ResourceLimits::default(),
            mounts: None, // Will be set to comprehensive mounts in ContainerSandbox::new
            work_dir: PathBuf::from("/w"), // Use /w like go-judge
            env: HashMap::new(),
            networking: false,
        }
    }
}

impl ContainerConfig {
    /// Create a new container configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set namespace configuration
    pub fn with_namespaces(mut self, namespaces: NamespaceConfig) -> Self {
        self.namespaces = namespaces;
        self
    }

    /// Set resource limits
    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Set mount configuration
    pub fn with_mounts(mut self, mounts: MountConfig) -> Self {
        self.mounts = Some(mounts);
        self
    }

    /// Set working directory
    pub fn with_work_dir<P: AsRef<Path>>(mut self, work_dir: P) -> Self {
        self.work_dir = work_dir.as_ref().to_path_buf();
        self
    }

    /// Add environment variable
    pub fn with_env<K: AsRef<str>, V: AsRef<str>>(mut self, key: K, value: V) -> Self {
        self.env
            .insert(key.as_ref().to_string(), value.as_ref().to_string());
        self
    }

    /// Enable or disable networking
    pub fn with_networking(mut self, networking: bool) -> Self {
        self.networking = networking;
        if !networking {
            self.namespaces.network = true; // Isolate network if disabled
        }
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        self.limits.validate()?;
        Ok(())
    }
}

/// Container sandbox for secure task execution
pub struct ContainerSandbox {
    config: ContainerConfig,
    root_dir: TempDir,
    namespace_manager: NamespaceManager,
}

impl ContainerSandbox {
    /// Create a new container sandbox
    pub fn new(config: ContainerConfig) -> Result<Self> {
        config.validate()?;

        let root_dir = tempfile::tempdir().map_err(|e| {
            SandboxError::ContainerCreation(format!("Failed to create root directory: {e}"))
        })?;

        let namespace_manager = NamespaceManager::new(config.namespaces.clone());

        info!(
            "Created container sandbox with root: {}",
            root_dir.path().display()
        );
        debug!("Container config: {:?}", config);
        debug!("Namespace config: {:?}", config.namespaces);
        info!(
            "DEBUG: Mount namespace enabled: {}",
            config.namespaces.mount
        );

        Ok(Self {
            config,
            root_dir,
            namespace_manager,
        })
    }
}

impl ContainerSandbox {
    /// Create a container with default configuration
    pub fn with_default_config() -> Result<Self> {
        Self::new(ContainerConfig::default())
    }

    /// Create a container for compilation tasks
    pub fn compilation() -> Result<Self> {
        let config = ContainerConfig::new()
            .with_limits(ResourceLimits::compilation())
            .with_namespaces(NamespaceConfig::default())
            .with_mounts(MountConfig::comprehensive_mounts("/")); // Use comprehensive mounts
        Self::new(config)
    }

    /// Create a container for execution tasks
    pub fn execution() -> Result<Self> {
        let config = ContainerConfig::new()
            .with_limits(ResourceLimits::execution())
            .with_namespaces(NamespaceConfig::default())
            .with_mounts(MountConfig::comprehensive_mounts("/")); // Use comprehensive mounts
        Self::new(config)
    }

    /// Get the root directory path
    pub fn root_path(&self) -> &Path {
        self.root_dir.path()
    }

    /// Get the working directory path inside container
    pub fn work_path(&self) -> PathBuf {
        self.root_path().join(
            self.config
                .work_dir
                .strip_prefix("/")
                .unwrap_or(&self.config.work_dir),
        )
    }

    /// Setup the container filesystem
    pub fn setup_filesystem(&self) -> Result<()> {
        info!("Setting up container filesystem");

        // Create working directory
        let work_path = self.work_path();
        std::fs::create_dir_all(&work_path).map_err(|e| {
            SandboxError::ContainerCreation(format!("Failed to create work directory: {e}"))
        })?;

        // Setup mounts if configured
        if let Some(ref mount_config) = self.config.mounts {
            mount_config.apply_mounts()?;
        } else {
            // Use comprehensive mounts by default (like go-judge)
            let comprehensive_mounts = MountConfig::comprehensive_mounts(self.root_path());
            comprehensive_mounts.apply_mounts()?;
        }

        info!("Container filesystem setup complete");
        Ok(())
    }

    /// Execute a command in the container
    pub async fn execute_command(
        &self,
        args: &[String],
        env: &[String],
        input: Option<&str>,
    ) -> Result<(ExitStatus, ResourceUsage, String, String)> {
        if args.is_empty() {
            return Err(SandboxError::ExecutionFailed(
                "No command provided".to_string(),
            ));
        }

        info!("Executing command in container: {:?}", args);
        debug!("Environment: {:?}", env);

        let start_time = Instant::now();
        let mut resource_usage = ResourceUsage::new();

        // Enter namespaces first (before filesystem setup)
        self.namespace_manager.enter_namespaces()?;

        // Setup filesystem within the mount namespace
        // self.setup_filesystem()?; // Temporarily disabled for testing

        // Apply resource limits
        self.config.limits.apply_rlimits()?;

        // Create and configure command
        let mut command = Command::new(&args[0]);

        // Set working directory to where files are copied
        // let work_path = self.work_path();
        // command.current_dir(&work_path); // Temporarily disabled for testing

        // Add arguments
        for arg in &args[1..] {
            command.arg(arg);
        }

        // Set environment variables
        command.env_clear(); // Clear inherited environment

        // Add basic required environment
        command.env("PATH", "/usr/local/bin:/usr/bin:/bin");
        command.env("HOME", "/tmp");
        command.env("USER", "sandbox");

        // Add configured environment variables
        for (key, value) in &self.config.env {
            command.env(key, value);
        }

        // Add provided environment variables
        for env_var in env {
            if let Some((key, value)) = env_var.split_once('=') {
                command.env(key, value);
            }
        }

        // Configure I/O
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        if input.is_some() {
            command.stdin(Stdio::piped());
        }

        // Spawn the process
        let mut child = command.spawn().map_err(|e| {
            SandboxError::ExecutionFailed(format!("Failed to spawn command: {}", e))
        })?;

        // Handle stdin if provided
        if let Some(input_data) = input {
            if let Some(stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let mut stdin = stdin;
                stdin.write_all(input_data.as_bytes()).await.map_err(|e| {
                    SandboxError::ExecutionFailed(format!("Failed to write stdin: {}", e))
                })?;
                stdin.shutdown().await.map_err(|e| {
                    SandboxError::ExecutionFailed(format!("Failed to close stdin: {}", e))
                })?;
            }
        }

        // Wait for completion with timeout
        let output = tokio::time::timeout(
            self.config.limits.wall_time_duration(),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| SandboxError::ExecutionFailed("Command timed out".to_string()))?
        .map_err(|e| SandboxError::ExecutionFailed(format!("Failed to wait for command: {}", e)))?;

        let elapsed = start_time.elapsed();
        resource_usage = resource_usage.with_wall_time(elapsed.as_nanos() as u64);

        // Convert output to strings
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Update resource usage with output size
        let output_size = (stdout.len() + stderr.len()) as u64;
        resource_usage = resource_usage.with_output_size(output_size);

        // Check if output size limit exceeded
        if output_size > self.config.limits.output {
            warn!(
                "Output size limit exceeded: {} > {}",
                output_size, self.config.limits.output
            );
            return Err(SandboxError::ExecutionFailed(
                "Output size limit exceeded".to_string(),
            ));
        }

        info!(
            "Command execution completed with status: {:?}",
            output.status
        );
        debug!("Resource usage: {}", resource_usage.summary());

        Ok((output.status, resource_usage, stdout, stderr))
    }

    /// Copy a file into the container
    pub fn copy_file<P: AsRef<Path>>(&self, filename: P, content: &str) -> Result<()> {
        let file_path = self.work_path().join(filename.as_ref());

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SandboxError::Io(e))?;
        }

        std::fs::write(&file_path, content).map_err(|e| SandboxError::Io(e))?;

        debug!("Copied file to container: {}", file_path.display());
        Ok(())
    }

    /// Read a file from the container
    pub fn read_file<P: AsRef<Path>>(&self, filename: P) -> Result<String> {
        let file_path = self.work_path().join(filename.as_ref());

        std::fs::read_to_string(&file_path).map_err(|e| SandboxError::Io(e))
    }

    /// List files in the container working directory
    pub fn list_files(&self) -> Result<Vec<String>> {
        let work_path = self.work_path();

        let entries = std::fs::read_dir(&work_path).map_err(|e| SandboxError::Io(e))?;

        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| SandboxError::Io(e))?;
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }

        files.sort();
        Ok(files)
    }

    /// Cleanup the container
    pub fn cleanup(&self) -> Result<()> {
        info!("Cleaning up container");

        // Cleanup mounts if they exist
        if let Some(ref mount_config) = self.config.mounts {
            if let Err(e) = mount_config.cleanup_mounts() {
                warn!("Failed to cleanup mounts: {}", e);
            }
        }

        // TempDir will be automatically cleaned up when dropped
        info!("Container cleanup completed");
        Ok(())
    }

    /// Get container configuration
    pub fn config(&self) -> &ContainerConfig {
        &self.config
    }
}

impl Drop for ContainerSandbox {
    fn drop(&mut self) {
        if let Err(e) = self.cleanup() {
            error!("Failed to cleanup container during drop: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_config_default() {
        let config = ContainerConfig::default();
        assert_eq!(config.work_dir, PathBuf::from("/w"));
        assert!(!config.networking);
        assert!(config.env.is_empty());
    }

    #[test]
    fn test_container_config_builder() {
        let config = ContainerConfig::new()
            .with_work_dir("/custom")
            .with_env("TEST", "value")
            .with_networking(true);

        assert_eq!(config.work_dir, PathBuf::from("/custom"));
        assert!(config.networking);
        assert_eq!(config.env.get("TEST"), Some(&"value".to_string()));
    }

    #[test]
    fn test_container_config_validation() {
        let valid_config = ContainerConfig::default();
        assert!(valid_config.validate().is_ok());

        let invalid_config = ContainerConfig::default().with_limits(ResourceLimits {
            cpu_time: 0,
            ..ResourceLimits::default()
        });
        assert!(invalid_config.validate().is_err());
    }

    #[tokio::test]
    async fn test_container_sandbox_creation() {
        let config = ContainerConfig::default();
        let sandbox = ContainerSandbox::new(config);
        assert!(sandbox.is_ok());

        let sandbox = sandbox.unwrap();
        assert!(sandbox.root_path().exists());
        assert!(sandbox.work_path().to_string_lossy().contains("sandbox"));
    }

    #[tokio::test]
    async fn test_container_file_operations() {
        let sandbox = ContainerSandbox::with_default_config().unwrap();

        // Copy a file
        let content = "Hello, Container!";
        sandbox.copy_file("test.txt", content).unwrap();

        // Read it back
        let read_content = sandbox.read_file("test.txt").unwrap();
        assert_eq!(read_content, content);

        // List files
        let files = sandbox.list_files().unwrap();
        assert!(files.contains(&"test.txt".to_string()));
    }

    #[tokio::test]
    async fn test_container_presets() {
        let compilation_sandbox = ContainerSandbox::compilation().unwrap();
        assert!(compilation_sandbox.config.limits.cpu_time > ResourceLimits::default().cpu_time);

        let execution_sandbox = ContainerSandbox::execution().unwrap();
        assert!(execution_sandbox.config.limits.memory < ResourceLimits::compilation().memory);
    }
}
