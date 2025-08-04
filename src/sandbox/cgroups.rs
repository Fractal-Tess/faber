//! Cgroups (Control Groups) management for resource limits
//!
//! This module provides functionality to create and manage cgroups for
//! enforcing resource limits on containerized processes.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info};

use super::error::SandboxError;
use crate::sandbox::container::ResourceLimits;

/// Resource usage statistics
#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub memory_usage: u64,
    pub cpu_usage: u64,
    pub process_count: u32,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

/// Cgroup manager for resource control
pub struct CgroupManager {
    /// Path to the cgroup directory
    cgroup_path: PathBuf,
    /// Whether the cgroup is active
    is_active: bool,
    /// Cgroup version (v1 or v2)
    version: CgroupVersion,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CgroupVersion {
    V1,
    V2,
}

impl CgroupManager {
    /// Create a new cgroup manager
    pub fn new(container_id: &str) -> Result<Self, SandboxError> {
        // Determine cgroup base path and version
        let (cgroup_base, version) = Self::find_cgroup_base()?;
        let cgroup_path = cgroup_base.join("faber").join(container_id);

        info!(
            "Creating cgroup at {} (version: {:?})",
            cgroup_path.display(),
            version
        );

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
            version,
        })
    }

    /// Find the cgroup base directory and determine version
    fn find_cgroup_base() -> Result<(PathBuf, CgroupVersion), SandboxError> {
        // Try cgroup v2 first
        if Path::new("/sys/fs/cgroup").exists() {
            // Check if it's cgroup v2
            if Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
                return Ok((PathBuf::from("/sys/fs/cgroup"), CgroupVersion::V2));
            }
        }

        // Try cgroup v1
        if Path::new("/sys/fs/cgroup/memory").exists() {
            return Ok((PathBuf::from("/sys/fs/cgroup/memory"), CgroupVersion::V1));
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
            "Applying resource limits to cgroup {} (version: {:?})",
            self.cgroup_path.display(),
            self.version
        );

        match self.version {
            CgroupVersion::V1 => self.apply_limits_v1(limits),
            CgroupVersion::V2 => self.apply_limits_v2(limits),
        }
    }

    /// Apply limits for cgroup v1
    fn apply_limits_v1(&self, limits: &ResourceLimits) -> Result<(), SandboxError> {
        // Memory limit
        self.set_memory_limit_v1(limits.memory_limit)?;

        // CPU limit
        self.set_cpu_limit_v1(limits.cpu_time_limit)?;

        // Process limit
        self.set_process_limit_v1(limits.max_processes)?;

        // File descriptor limit
        self.set_fd_limit_v1(limits.max_fds)?;

        debug!("Cgroup v1 resource limits applied successfully");
        Ok(())
    }

    /// Apply limits for cgroup v2
    fn apply_limits_v2(&self, limits: &ResourceLimits) -> Result<(), SandboxError> {
        // Memory limit
        self.set_memory_limit_v2(limits.memory_limit)?;

        // CPU limit
        self.set_cpu_limit_v2(limits.cpu_time_limit)?;

        // Process limit
        self.set_process_limit_v2(limits.max_processes)?;

        // File descriptor limit
        self.set_fd_limit_v2(limits.max_fds)?;

        debug!("Cgroup v2 resource limits applied successfully");
        Ok(())
    }

    /// Set memory limit in bytes (cgroup v1)
    fn set_memory_limit_v1(&self, limit_bytes: u64) -> Result<(), SandboxError> {
        let memory_limit_path = self.cgroup_path.join("memory.limit_in_bytes");
        let limit_str = limit_bytes.to_string();

        fs::write(&memory_limit_path, &limit_str).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to set memory limit {} in {}: {}",
                limit_str,
                memory_limit_path.display(),
                e
            ))
        })?;

        // Also set memory+swap limit to prevent swapping
        let memsw_limit_path = self.cgroup_path.join("memory.memsw.limit_in_bytes");
        if memsw_limit_path.exists() {
            fs::write(&memsw_limit_path, &limit_str).map_err(|e| {
                SandboxError::ResourceLimitFailed(format!("Failed to set memory+swap limit: {}", e))
            })?;
        }

        Ok(())
    }

    /// Set memory limit in bytes (cgroup v2)
    fn set_memory_limit_v2(&self, limit_bytes: u64) -> Result<(), SandboxError> {
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

        Ok(())
    }

    /// Set CPU limit (cgroup v1)
    fn set_cpu_limit_v1(&self, limit_ns: u64) -> Result<(), SandboxError> {
        // Convert nanoseconds to microseconds for cgroup
        let limit_us = limit_ns / 1000;
        let cpu_limit_path = self.cgroup_path.join("cpu.cfs_quota_us");
        let limit_str = limit_us.to_string();

        fs::write(&cpu_limit_path, &limit_str).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to set CPU limit {} in {}: {}",
                limit_str,
                cpu_limit_path.display(),
                e
            ))
        })?;

        Ok(())
    }

    /// Set CPU limit (cgroup v2)
    fn set_cpu_limit_v2(&self, limit_ns: u64) -> Result<(), SandboxError> {
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

        Ok(())
    }

    /// Set process limit (cgroup v1)
    fn set_process_limit_v1(&self, max_processes: u32) -> Result<(), SandboxError> {
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

        Ok(())
    }

    /// Set process limit (cgroup v2)
    fn set_process_limit_v2(&self, max_processes: u32) -> Result<(), SandboxError> {
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

        Ok(())
    }

    /// Set file descriptor limit (cgroup v1)
    fn set_fd_limit_v1(&self, _max_fds: u64) -> Result<(), SandboxError> {
        // Cgroup v1 doesn't have direct FD limits, use rlimit instead
        // This is handled in the container setup
        Ok(())
    }

    /// Set file descriptor limit (cgroup v2)
    fn set_fd_limit_v2(&self, _max_fds: u64) -> Result<(), SandboxError> {
        // Cgroup v2 doesn't have direct FD limits, use rlimit instead
        // This is handled in the container setup
        Ok(())
    }

    /// Add a process to the cgroup
    pub fn add_process(&self, pid: u32) -> Result<(), SandboxError> {
        if !self.is_active {
            return Ok(());
        }

        let tasks_path = match self.version {
            CgroupVersion::V1 => self.cgroup_path.join("tasks"),
            CgroupVersion::V2 => self.cgroup_path.join("cgroup.procs"),
        };

        let pid_str = pid.to_string();
        fs::write(&tasks_path, &pid_str).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to add process {} to cgroup {}: {}",
                pid,
                tasks_path.display(),
                e
            ))
        })?;

        debug!(
            "Added process {} to cgroup {}",
            pid,
            self.cgroup_path.display()
        );
        Ok(())
    }

    /// Get current resource usage statistics
    pub fn get_resource_stats(&self) -> Result<ResourceStats, SandboxError> {
        if !self.is_active {
            return Err(SandboxError::ResourceLimitFailed(
                "Cgroup is not active".to_string(),
            ));
        }

        let memory_usage = self.get_memory_usage()?;
        let cpu_usage = self.get_cpu_usage()?;
        let process_count = self.get_process_count()?;
        let (io_read, io_write) = self.get_io_stats()?;

        Ok(ResourceStats {
            memory_usage,
            cpu_usage,
            process_count,
            io_read_bytes: io_read,
            io_write_bytes: io_write,
        })
    }

    /// Get memory usage in bytes
    fn get_memory_usage(&self) -> Result<u64, SandboxError> {
        let memory_usage_path = match self.version {
            CgroupVersion::V1 => self.cgroup_path.join("memory.usage_in_bytes"),
            CgroupVersion::V2 => self.cgroup_path.join("memory.current"),
        };

        let usage_str = fs::read_to_string(&memory_usage_path).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to read memory usage from {}: {}",
                memory_usage_path.display(),
                e
            ))
        })?;

        usage_str.trim().parse::<u64>().map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to parse memory usage '{}': {}",
                usage_str, e
            ))
        })
    }

    /// Get CPU usage in nanoseconds
    fn get_cpu_usage(&self) -> Result<u64, SandboxError> {
        let cpu_usage_path = match self.version {
            CgroupVersion::V1 => self.cgroup_path.join("cpuacct.usage"),
            CgroupVersion::V2 => self.cgroup_path.join("cpu.stat"),
        };

        if self.version == CgroupVersion::V1 {
            let usage_str = fs::read_to_string(&cpu_usage_path).map_err(|e| {
                SandboxError::ResourceLimitFailed(format!(
                    "Failed to read CPU usage from {}: {}",
                    cpu_usage_path.display(),
                    e
                ))
            })?;

            usage_str.trim().parse::<u64>().map_err(|e| {
                SandboxError::ResourceLimitFailed(format!(
                    "Failed to parse CPU usage '{}': {}",
                    usage_str, e
                ))
            })
        } else {
            // For cgroup v2, parse cpu.stat file
            let stat_content = fs::read_to_string(&cpu_usage_path).map_err(|e| {
                SandboxError::ResourceLimitFailed(format!(
                    "Failed to read CPU stats from {}: {}",
                    cpu_usage_path.display(),
                    e
                ))
            })?;

            // Parse "usage_usec" line
            for line in stat_content.lines() {
                if line.starts_with("usage_usec") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() == 2 {
                        return parts[1]
                            .parse::<u64>()
                            .map(|usec| usec * 1000)
                            .map_err(|e| {
                                SandboxError::ResourceLimitFailed(format!(
                                    "Failed to parse CPU usage: {}",
                                    e
                                ))
                            });
                    }
                }
            }

            Ok(0) // Default if not found
        }
    }

    /// Get process count
    fn get_process_count(&self) -> Result<u32, SandboxError> {
        let pids_current_path = self.cgroup_path.join("pids.current");
        let count_str = fs::read_to_string(&pids_current_path).map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to read process count from {}: {}",
                pids_current_path.display(),
                e
            ))
        })?;

        count_str.trim().parse::<u32>().map_err(|e| {
            SandboxError::ResourceLimitFailed(format!(
                "Failed to parse process count '{}': {}",
                count_str, e
            ))
        })
    }

    /// Get I/O statistics
    fn get_io_stats(&self) -> Result<(u64, u64), SandboxError> {
        // I/O stats are not available in all cgroup setups
        // Return zeros for now
        Ok((0, 0))
    }

    /// Get the cgroup path
    pub fn path(&self) -> &Path {
        &self.cgroup_path
    }

    /// Check if the cgroup is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Clean up the cgroup
    pub fn cleanup(&mut self) -> Result<(), SandboxError> {
        if !self.is_active {
            return Ok(());
        }

        info!("Cleaning up cgroup {}", self.cgroup_path.display());

        // Remove all processes from the cgroup
        let tasks_path = match self.version {
            CgroupVersion::V1 => self.cgroup_path.join("tasks"),
            CgroupVersion::V2 => self.cgroup_path.join("cgroup.procs"),
        };

        if tasks_path.exists() {
            if let Ok(content) = fs::read_to_string(&tasks_path) {
                for line in content.lines() {
                    if let Ok(_pid) = line.trim().parse::<u32>() {
                        // Move process to root cgroup
                        let _ = fs::write("/sys/fs/cgroup/tasks", line);
                    }
                }
            }
        }

        // Remove the cgroup directory
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
        debug!("Cgroup cleanup completed");
        Ok(())
    }
}

impl Drop for CgroupManager {
    fn drop(&mut self) {
        if self.is_active {
            if let Err(e) = self.cleanup() {
                error!("Failed to cleanup cgroup during drop: {:?}", e);
            }
        }
    }
}
