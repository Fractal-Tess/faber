//! Container runtime management and lifecycle operations.
//!
//! This module provides the high-level `ContainerRuntime` interface for managing
//! container instances throughout their lifecycle. It orchestrates filesystem setup,
//! file management, and isolated command execution.
//!
//! # Container Lifecycle
//!
//! A typical container lifecycle follows this pattern:
//!
//! 1. **Creation**: `ContainerRuntime::new()` - Initialize runtime with configuration
//! 2. **File Writing**: `write_files()` - Add user files to work directory
//! 3. **Execution**: `run_command()` - Execute commands in isolation
//! 4. **Cleanup**: Automatic via Drop (or call `cleanup()` manually)
//!
//! # Security Model
//!
//! The container runtime implements multiple layers of security:
//! - **Filesystem isolation**: chroot to container root directory
//! - **Namespace isolation**: Separate PID, mount, network, IPC, and UTS namespaces
//! - **Mount restrictions**: nosuid, nodev, and optional read-only mounts
//! - **Permission control**: Restricted directory permissions (0o700)
//!
//! # Example Usage
//!
//! ```rust
//! use std::collections::HashMap;
//!
//! // Create container
//! let runtime = ContainerRuntime::new(fs_config, "request-123");
//!
//! // Add files and execute command
//! let files = HashMap::from([("main.py".to_string(), "print('Hello')".to_string())]);
//! runtime.write_files(&files)?;
//! let (stdout, stderr, exit_code) = runtime.run_command("python3", &["main.py".to_string()], &HashMap::new()).await?;
//!
//! // Cleanup happens automatically when `runtime` is dropped
//! ```

use super::errors::ContainerError;
use super::mounts;
use crate::config::ContainerFilesystemConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    pub request_id: String,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct ContainerRuntime {
    /// Unique identifier for this container instance
    request_id: String,
    /// Absolute path to the container's root directory on the host
    root: PathBuf,
    /// Filesystem configuration specifying mounts and directories
    fs_cfg: ContainerFilesystemConfig,
    /// Whether cleanup has already been attempted (shared across clones)
    cleanup_done: Arc<AtomicBool>,
}

impl ContainerRuntime {
    /// Creates a new container runtime instance without touching the filesystem.
    pub fn new(fs_cfg: ContainerFilesystemConfig, request_id: impl Into<String>) -> Self {
        let request_id = request_id.into();
        let root = Path::new(&fs_cfg.base_dir).join(&request_id);
        debug!("Container root: {}", root.display());
        Self {
            request_id,
            root,
            fs_cfg,
            cleanup_done: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Executes a vector of tasks sequentially inside this container runtime and returns their results.
    pub async fn run_tasks(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>, ContainerError> {
        let request_id = self.request_id.clone();
        let root = self.root.clone();
        let work_dir = self.fs_cfg.work_dir.target.clone();
        let fs_cfg = self.fs_cfg.clone();

        let request_id_for_spawn = request_id.clone();

        tokio::task::spawn_blocking(move || {
            Self::run_tasks_prepared_isolated_blocking(
                &request_id_for_spawn,
                &root,
                &work_dir,
                tasks,
                &fs_cfg,
            )
        })
        .await
        .map_err(|e| ContainerError::Spawn {
            cmd: "run_tasks".to_string(),
            source: std::io::Error::other(format!("Join error: {e}")),
        })?
    }

    /// Cleans up the container by removing directories (mounts are in child namespaces).
    pub fn cleanup(&self) -> Result<(), ContainerError> {
        info!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Cleaning up container"
        );
        self.perform_cleanup()?;
        debug!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Container cleaned up"
        );
        Ok(())
    }

    /// Performs idempotent cleanup without logging and without returning errors to Drop callers.
    fn perform_cleanup(&self) -> Result<(), ContainerError> {
        // Ensure this runs at most once across all clones
        if self.cleanup_done.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        // Remove the container root directory; ignore if it doesn't exist
        if self.root.exists() {
            fs::remove_dir_all(&self.root).map_err(|e| ContainerError::RemoveDir {
                path: self.root.clone(),
                source: e,
            })?;
        }

        Ok(())
    }

    /// Returns the absolute path to the container's root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes user-provided files into the container's work directory.
    pub fn write_files(&self, files: &HashMap<String, String>) -> Result<(), ContainerError> {
        info!(
            request_id = %self.request_id,
            root = %self.root.display(),
            file_count = files.len(),
            "Writing files into container work directory"
        );
        if files.is_empty() {
            return Ok(());
        }
        let host_work_dir = self.root.join(&self.fs_cfg.work_dir.target);
        for (relative_path, content) in files {
            let dest = host_work_dir.join(relative_path);
            debug!(
                request_id = %self.request_id,
                path = %dest.display(),
                bytes = content.len(),
                "Writing file"
            );
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| ContainerError::CreateDir {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            fs::write(&dest, content).map_err(|e| ContainerError::WriteFile {
                path: dest.clone(),
                source: e,
            })?;
        }
        debug!(
            request_id = %self.request_id,
            root = %self.root.display(),
            "Finished writing files"
        );
        Ok(())
    }

    /// Executes a command within the isolated container environment.
    pub async fn run_command(
        &self,
        cmd: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<(String, String, i32), ContainerError> {
        let cmd_string = cmd.to_string();
        let args_vec = args.to_vec();
        let env_map = env.clone();
        let request_id = self.request_id.clone();
        let root = self.root.clone();
        let work_dir = self.fs_cfg.work_dir.target.clone();
        let fs_cfg = self.fs_cfg.clone();

        tokio::task::spawn_blocking(move || {
            Self::run_isolated_blocking(
                &request_id,
                &root,
                &work_dir,
                &cmd_string,
                &args_vec,
                &env_map,
                &fs_cfg,
            )
        })
        .await
        .map_err(|e| ContainerError::Spawn {
            cmd: cmd.to_string(),
            source: std::io::Error::other(format!("Join error: {e}")),
        })?
    }
}

impl Drop for ContainerRuntime {
    fn drop(&mut self) {
        if let Err(err) = self.perform_cleanup() {
            warn!(
                request_id = %self.request_id,
                root = %self.root.display(),
                error = %err,
                "Cleanup during Drop failed"
            );
        }
    }
}
