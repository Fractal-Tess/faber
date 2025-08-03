use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::debug;
use uuid::Uuid;

use super::error::SandboxError;

pub struct Sandbox {
    /// Unique identifier for this sandbox instance
    sandbox_id: String,
    /// Working directory where files are copied and tasks run
    work_dir: PathBuf,
    /// Whether the sandbox is still active
    is_active: bool,
}

impl Sandbox {
    pub fn new() -> Result<Self, SandboxError> {
        let sandbox_id = Uuid::new_v4().to_string();
        debug!("Creating new sandbox with ID: {}", sandbox_id);

        // Create a unique temporary directory for this sandbox
        let work_dir = std::env::temp_dir().join("faber_sandbox").join(&sandbox_id);

        // Create the directory
        std::fs::create_dir_all(&work_dir).map_err(|e| {
            SandboxError::ContainerCreation(format!(
                "Failed to create sandbox directory {}: {}",
                work_dir.display(),
                e
            ))
        })?;

        debug!("Created sandbox directory: {}", work_dir.display());

        Ok(Self {
            sandbox_id,
            work_dir,
            is_active: true,
        })
    }

    /// Get the working directory for this sandbox
    pub fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    /// Get the sandbox ID
    pub fn sandbox_id(&self) -> &str {
        &self.sandbox_id
    }

    /// Check if the sandbox is still active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Copy files into the sandbox working directory
    /// files: HashMap where key is the file path inside sandbox, value is the file content
    pub fn copy_files_in(&mut self, files: &HashMap<String, String>) -> Result<(), SandboxError> {
        if !self.is_active {
            return Err(SandboxError::ExecutionFailed(
                "Cannot copy files to inactive sandbox".to_string(),
            ));
        }

        debug!(
            "Copying {} files into sandbox {}",
            files.len(),
            self.sandbox_id
        );

        for (file_path, content) in files {
            let full_path = self.work_dir.join(file_path);

            // Create parent directories if they don't exist
            if let Some(parent) = full_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    SandboxError::ExecutionFailed(format!(
                        "Failed to create directory {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }

            // Write the file content
            std::fs::write(&full_path, content).map_err(|e| {
                SandboxError::ExecutionFailed(format!(
                    "Failed to write file {}: {}",
                    full_path.display(),
                    e
                ))
            })?;

            debug!("Copied file: {}", file_path);
        }

        debug!("Successfully copied all files into sandbox");
        Ok(())
    }

    /// Get environment variables for process execution
    pub fn execution_environment(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Set the working directory
        env.insert(
            "PWD".to_string(),
            self.work_dir.to_string_lossy().to_string(),
        );

        // Add basic isolation environment variables
        env.insert(
            "HOME".to_string(),
            self.work_dir.to_string_lossy().to_string(),
        );
        env.insert(
            "TMPDIR".to_string(),
            self.work_dir.to_string_lossy().to_string(),
        );

        env
    }

    /// Apply resource limits and security settings to a process command
    /// This method configures the command to run with appropriate limits
    pub fn apply_limits(&self, cmd: &mut Command) -> Result<(), SandboxError> {
        if !self.is_active {
            return Err(SandboxError::ExecutionFailed(
                "Cannot apply limits to command in inactive sandbox".to_string(),
            ));
        }

        debug!("Applying limits to command in sandbox {}", self.sandbox_id);

        // Set the working directory for the command
        cmd.current_dir(&self.work_dir);

        // Apply environment variables
        let env = self.execution_environment();
        for (key, value) in env {
            cmd.env(key, value);
        }

        // Clear inherited environment to provide better isolation
        cmd.env_clear();

        // Add back essential environment variables
        cmd.env("PATH", "/usr/local/bin:/usr/bin:/bin");
        cmd.env("PWD", self.work_dir.to_string_lossy().to_string());
        cmd.env("HOME", self.work_dir.to_string_lossy().to_string());
        cmd.env("TMPDIR", self.work_dir.to_string_lossy().to_string());

        // TODO: Add resource limits (CPU, memory, time) using process groups
        // TODO: Add namespace isolation (PID, mount, network)
        // TODO: Add seccomp filtering for syscall restrictions

        debug!("Applied basic limits and environment isolation");
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<(), SandboxError> {
        if !self.is_active {
            debug!("Sandbox {} already cleaned up", self.sandbox_id);
            return Ok(());
        }

        debug!("Cleaning up sandbox {}", self.sandbox_id);

        // Remove the working directory and all its contents
        if self.work_dir.exists() {
            std::fs::remove_dir_all(&self.work_dir).map_err(|e| {
                SandboxError::CleanupFailed(format!(
                    "Failed to remove sandbox directory {}: {}",
                    self.work_dir.display(),
                    e
                ))
            })?;
            debug!("Removed sandbox directory: {}", self.work_dir.display());
        }

        self.is_active = false;
        Ok(())
    }
}
