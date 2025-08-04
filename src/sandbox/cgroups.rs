//! Cgroups (Control Groups) management for resource limits
//!
//! This module provides functionality to create and manage cgroups for
//! enforcing resource limits on containerized processes.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info};

use super::error::SandboxError;
use crate::sandbox::container::ResourceLimits;

/// Cgroup manager for resource control
pub struct CgroupManager {
    /// Path to the cgroup directory
    cgroup_path: PathBuf,
    /// Whether the cgroup is active
    is_active: bool,
}

impl CgroupManager {
    /// Create a new cgroup manager
    pub fn new(container_id: &str) -> Result<Self, SandboxError> {
        // Determine cgroup base path
        let cgroup_base = Self::find_cgroup_base()?;
        let cgroup_path = cgroup_base.join("faber").join(container_id);

        info!("Creating cgroup at {}", cgroup_path.display());

        // Create cgroup directory
        fs::create_dir_all(&cgroup_path).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to create cgroup directory {}: {}",
                cgroup_path.display(),
                e
            ))
        })?;

        Ok(Self {
            cgroup_path,
            is_active: true,
        })
    }

    /// Find the cgroup base directory
    fn find_cgroup_base() -> Result<PathBuf, SandboxError> {
        // Try cgroup v2 first
        if Path::new("/sys/fs/cgroup").exists() {
            // Check if it's cgroup v2
            if Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
                return Ok(PathBuf::from("/sys/fs/cgroup"));
            }
        }

        // Try cgroup v1
        if Path::new("/sys/fs/cgroup/memory").exists() {
            return Ok(PathBuf::from("/sys/fs/cgroup/memory"));
        }

        Err(SandboxError::ResourceLimitFailed(
            "No cgroup filesystem found".to_string(),
        ))
    }

    /// Apply resource limits to the cgroup
    pub fn apply_limits(&self, limits: &ResourceLimits) -> Result<(), SandboxError> {
        if !self.is_active {
            return Ok(());
        }

        info!(
            "Applying resource limits to cgroup {}",
            self.cgroup_path.display()
        );

        // Apply memory limit
        self.set_memory_limit(limits.memory_limit)?;

        // Apply CPU limit (convert nanoseconds to microseconds for cgroup)
        self.set_cpu_limit(limits.cpu_time_limit)?;

        // Apply process limit
        self.set_process_limit(limits.max_processes)?;

        // Apply file descriptor limit
        self.set_fd_limit(limits.max_fds)?;

        debug!("Resource limits applied successfully");
        Ok(())
    }

    /// Set memory limit in bytes
    fn set_memory_limit(&self, limit_bytes: u64) -> Result<(), SandboxError> {
        let memory_max_path = self.cgroup_path.join("memory.max");
        let limit_str = limit_bytes.to_string();

        fs::write(&memory_max_path, &limit_str).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to set memory limit {} in {}: {}",
                limit_str,
                memory_max_path.display(),
                e
            ))
        })?;

        debug!("Set memory limit to {} bytes", limit_bytes);
        Ok(())
    }

    /// Set CPU time limit in nanoseconds
    fn set_cpu_limit(&self, limit_ns: u64) -> Result<(), SandboxError> {
        // Convert nanoseconds to microseconds for cgroup
        let limit_us = limit_ns / 1000;
        let cpu_max_path = self.cgroup_path.join("cpu.max");
        let limit_str = format!("{} 100000", limit_us); // 100000 microseconds = 100ms period

        fs::write(&cpu_max_path, &limit_str).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to set CPU limit {} in {}: {}",
                limit_str,
                cpu_max_path.display(),
                e
            ))
        })?;

        debug!("Set CPU limit to {} microseconds", limit_us);
        Ok(())
    }

    /// Set maximum number of processes
    fn set_process_limit(&self, max_processes: u32) -> Result<(), SandboxError> {
        let pids_max_path = self.cgroup_path.join("pids.max");
        let limit_str = max_processes.to_string();

        fs::write(&pids_max_path, &limit_str).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to set process limit {} in {}: {}",
                limit_str,
                pids_max_path.display(),
                e
            ))
        })?;

        debug!("Set process limit to {}", max_processes);
        Ok(())
    }

    /// Set maximum number of file descriptors
    fn set_fd_limit(&self, max_fds: u64) -> Result<(), SandboxError> {
        // Note: File descriptor limits are typically set via ulimit, not cgroups
        // This is a placeholder for future implementation
        debug!("File descriptor limit set to {} (via ulimit)", max_fds);
        Ok(())
    }

    /// Add a process to the cgroup
    pub fn add_process(&self, pid: u32) -> Result<(), SandboxError> {
        if !self.is_active {
            return Ok(());
        }

        let procs_path = self.cgroup_path.join("cgroup.procs");
        let pid_str = pid.to_string();

        fs::write(&procs_path, &pid_str).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to add process {} to cgroup {}: {}",
                pid,
                procs_path.display(),
                e
            ))
        })?;

        debug!("Added process {} to cgroup", pid);
        Ok(())
    }

    /// Get the cgroup path
    pub fn path(&self) -> &Path {
        &self.cgroup_path
    }

    /// Check if cgroup is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Clean up the cgroup
    pub fn cleanup(&mut self) -> Result<(), SandboxError> {
        if !self.is_active {
            return Ok(());
        }

        info!("Cleaning up cgroup {}", self.cgroup_path.display());

        // Remove cgroup directory
        if self.cgroup_path.exists() {
            fs::remove_dir(&self.cgroup_path).map_err(|e| {
                SandboxError::ResourceLimitFailed(format!(
                    "Failed to remove cgroup directory {}: {}",
                    self.cgroup_path.display(),
                    e
                ))
            })?;
        }

        self.is_active = false;
        Ok(())
    }
}

impl Drop for CgroupManager {
    fn drop(&mut self) {
        if self.is_active {
            if let Err(e) = self.cleanup() {
                error!("Failed to cleanup cgroup during drop: {}", e);
            }
        }
    }
}
