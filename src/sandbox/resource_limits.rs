//! Resource limits for container execution
//!
//! This module provides functionality to set and enforce resource limits
//! on container processes, including CPU time, memory, and process limits.

use crate::sandbox::{Result, SandboxError};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// Resource limits for container execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU time limit in nanoseconds
    pub cpu_time: u64,
    /// Wall clock time limit in nanoseconds
    pub wall_time: u64,
    /// Memory limit in bytes
    pub memory: u64,
    /// Stack size limit in bytes
    pub stack: u64,
    /// Output size limit in bytes
    pub output: u64,
    /// Maximum number of processes
    pub processes: u32,
    /// Maximum number of open file descriptors
    pub fds: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_time: 5_000_000_000,   // 5 seconds
            wall_time: 10_000_000_000, // 10 seconds
            memory: 256 * 1024 * 1024, // 256MB
            stack: 8 * 1024 * 1024,    // 8MB
            output: 64 * 1024 * 1024,  // 64MB
            processes: 32,             // 32 processes
            fds: 64,                   // 64 file descriptors
        }
    }
}

impl ResourceLimits {
    /// Create new resource limits
    pub fn new(
        cpu_time: u64,
        wall_time: u64,
        memory: u64,
        stack: u64,
        output: u64,
        processes: u32,
        fds: u64,
    ) -> Self {
        Self {
            cpu_time,
            wall_time,
            memory,
            stack,
            output,
            processes,
            fds,
        }
    }

    /// Create resource limits for compilation tasks
    pub fn compilation() -> Self {
        Self {
            cpu_time: 30_000_000_000,   // 30 seconds
            wall_time: 60_000_000_000,  // 60 seconds
            memory: 1024 * 1024 * 1024, // 1GB
            stack: 16 * 1024 * 1024,    // 16MB
            output: 10 * 1024 * 1024,   // 10MB
            processes: 16,              // 16 processes
            fds: 32,                    // 32 file descriptors
        }
    }

    /// Create resource limits for execution tasks
    pub fn execution() -> Self {
        Self {
            cpu_time: 5_000_000_000,   // 5 seconds
            wall_time: 10_000_000_000, // 10 seconds
            memory: 128 * 1024 * 1024, // 128MB
            stack: 8 * 1024 * 1024,    // 8MB
            output: 64 * 1024 * 1024,  // 64MB
            processes: 8,              // 8 processes
            fds: 16,                   // 16 file descriptors
        }
    }

    /// Create resource limits for testing (very restrictive)
    pub fn testing() -> Self {
        Self {
            cpu_time: 1_000_000_000,  // 1 second
            wall_time: 2_000_000_000, // 2 seconds
            memory: 64 * 1024 * 1024, // 64MB
            stack: 4 * 1024 * 1024,   // 4MB
            output: 1024 * 1024,      // 1MB
            processes: 4,             // 4 processes
            fds: 8,                   // 8 file descriptors
        }
    }

    /// Get CPU time limit as Duration
    pub fn cpu_time_duration(&self) -> Duration {
        Duration::from_nanos(self.cpu_time)
    }

    /// Get wall time limit as Duration
    pub fn wall_time_duration(&self) -> Duration {
        Duration::from_nanos(self.wall_time)
    }

    /// Validate the resource limits
    pub fn validate(&self) -> Result<()> {
        if self.cpu_time == 0 {
            return Err(SandboxError::ResourceLimitFailed(
                "CPU time limit cannot be zero".to_string(),
            ));
        }
        if self.wall_time == 0 {
            return Err(SandboxError::ResourceLimitFailed(
                "Wall time limit cannot be zero".to_string(),
            ));
        }
        if self.memory == 0 {
            return Err(SandboxError::ResourceLimitFailed(
                "Memory limit cannot be zero".to_string(),
            ));
        }
        if self.processes == 0 {
            return Err(SandboxError::ResourceLimitFailed(
                "Process limit cannot be zero".to_string(),
            ));
        }

        // Wall time should be at least as long as CPU time
        if self.wall_time < self.cpu_time {
            return Err(SandboxError::ResourceLimitFailed(
                "Wall time limit should be at least as long as CPU time limit".to_string(),
            ));
        }

        Ok(())
    }

    /// Apply resource limits using rlimit
    pub fn apply_rlimits(&self) -> Result<()> {
        info!(
            "Applying resource limits: CPU={}s, Memory={}MB, Processes={}",
            self.cpu_time / 1_000_000_000,
            self.memory / (1024 * 1024),
            self.processes
        );

        // Set CPU time limit
        let cpu_seconds = (self.cpu_time / 1_000_000_000) as u64;
        set_rlimit(libc::RLIMIT_CPU, cpu_seconds)?;

        // Set memory limit (virtual memory)
        set_rlimit(libc::RLIMIT_AS, self.memory)?;

        // Set stack size limit
        set_rlimit(libc::RLIMIT_STACK, self.stack)?;

        // Set process limit
        set_rlimit(libc::RLIMIT_NPROC, self.processes as u64)?;

        // Set file descriptor limit
        set_rlimit(libc::RLIMIT_NOFILE, self.fds)?;

        // Set core dump size to 0 for security
        set_rlimit(libc::RLIMIT_CORE, 0)?;

        debug!("Successfully applied all resource limits");
        Ok(())
    }

    /// Get a human-readable summary of the limits
    pub fn summary(&self) -> String {
        format!(
            "CPU: {}s, Wall: {}s, Memory: {}MB, Stack: {}MB, Output: {}MB, Processes: {}, FDs: {}",
            self.cpu_time / 1_000_000_000,
            self.wall_time / 1_000_000_000,
            self.memory / (1024 * 1024),
            self.stack / (1024 * 1024),
            self.output / (1024 * 1024),
            self.processes,
            self.fds
        )
    }
}

/// Set a resource limit using rlimit
fn set_rlimit(resource: i32, limit: u64) -> Result<()> {
    let rlimit = libc::rlimit {
        rlim_cur: limit,
        rlim_max: limit,
    };

    let result = unsafe { libc::setrlimit(resource, &rlimit) };

    if result != 0 {
        let errno = std::io::Error::last_os_error();
        return Err(SandboxError::ResourceLimitFailed(format!(
            "Failed to set resource limit {}: {}",
            resource, errno
        )));
    }

    Ok(())
}

/// Resource usage tracking for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU time used in nanoseconds
    pub cpu_time: u64,
    /// Wall clock time used in nanoseconds
    pub wall_time: u64,
    /// Peak memory usage in bytes
    pub memory: u64,
    /// Peak number of processes
    pub process_peak: u64,
    /// Output size in bytes
    pub output_size: u64,
}

impl ResourceUsage {
    /// Create a new resource usage tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Update CPU time
    pub fn with_cpu_time(mut self, cpu_time: u64) -> Self {
        self.cpu_time = cpu_time;
        self
    }

    /// Update wall time
    pub fn with_wall_time(mut self, wall_time: u64) -> Self {
        self.wall_time = wall_time;
        self
    }

    /// Update memory usage
    pub fn with_memory(mut self, memory: u64) -> Self {
        self.memory = memory;
        self
    }

    /// Update process peak
    pub fn with_process_peak(mut self, process_peak: u64) -> Self {
        self.process_peak = process_peak;
        self
    }

    /// Update output size
    pub fn with_output_size(mut self, output_size: u64) -> Self {
        self.output_size = output_size;
        self
    }

    /// Check if any limits are exceeded
    pub fn check_limits(&self, limits: &ResourceLimits) -> Option<String> {
        if self.cpu_time > limits.cpu_time {
            return Some("CPU time limit exceeded".to_string());
        }
        if self.wall_time > limits.wall_time {
            return Some("Wall time limit exceeded".to_string());
        }
        if self.memory > limits.memory {
            return Some("Memory limit exceeded".to_string());
        }
        if self.process_peak > limits.processes as u64 {
            return Some("Process limit exceeded".to_string());
        }
        if self.output_size > limits.output {
            return Some("Output size limit exceeded".to_string());
        }
        None
    }

    /// Get a human-readable summary
    pub fn summary(&self) -> String {
        format!(
            "CPU: {}ms, Wall: {}ms, Memory: {}MB, Processes: {}, Output: {}KB",
            self.cpu_time / 1_000_000,
            self.wall_time / 1_000_000,
            self.memory / (1024 * 1024),
            self.process_peak,
            self.output_size / 1024
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu_time, 5_000_000_000);
        assert_eq!(limits.wall_time, 10_000_000_000);
        assert_eq!(limits.memory, 256 * 1024 * 1024);
        assert_eq!(limits.processes, 32);
    }

    #[test]
    fn test_resource_limits_compilation() {
        let limits = ResourceLimits::compilation();
        assert_eq!(limits.cpu_time, 30_000_000_000);
        assert_eq!(limits.memory, 1024 * 1024 * 1024);
        assert!(limits.cpu_time > ResourceLimits::default().cpu_time);
        assert!(limits.memory > ResourceLimits::default().memory);
    }

    #[test]
    fn test_resource_limits_execution() {
        let limits = ResourceLimits::execution();
        assert_eq!(limits.cpu_time, 5_000_000_000);
        assert_eq!(limits.memory, 128 * 1024 * 1024);
        assert!(limits.memory < ResourceLimits::compilation().memory);
    }

    #[test]
    fn test_resource_limits_validation() {
        let valid_limits = ResourceLimits::default();
        assert!(valid_limits.validate().is_ok());

        let invalid_limits = ResourceLimits {
            cpu_time: 0,
            ..ResourceLimits::default()
        };
        assert!(invalid_limits.validate().is_err());

        let invalid_wall_time = ResourceLimits {
            wall_time: 1_000_000_000,
            cpu_time: 5_000_000_000,
            ..ResourceLimits::default()
        };
        assert!(invalid_wall_time.validate().is_err());
    }

    #[test]
    fn test_resource_usage_builder() {
        let usage = ResourceUsage::new()
            .with_cpu_time(1_000_000_000)
            .with_memory(64 * 1024 * 1024)
            .with_process_peak(4);

        assert_eq!(usage.cpu_time, 1_000_000_000);
        assert_eq!(usage.memory, 64 * 1024 * 1024);
        assert_eq!(usage.process_peak, 4);
    }

    #[test]
    fn test_resource_usage_limit_check() {
        let limits = ResourceLimits::default();
        let usage = ResourceUsage::new().with_cpu_time(limits.cpu_time + 1_000_000_000); // Exceed CPU time

        let violation = usage.check_limits(&limits);
        assert!(violation.is_some());
        assert!(violation.unwrap().contains("CPU time"));
    }

    #[test]
    fn test_duration_conversion() {
        let limits = ResourceLimits::default();
        let cpu_duration = limits.cpu_time_duration();
        let wall_duration = limits.wall_time_duration();

        assert_eq!(cpu_duration, Duration::from_nanos(limits.cpu_time));
        assert_eq!(wall_duration, Duration::from_nanos(limits.wall_time));
    }
}
