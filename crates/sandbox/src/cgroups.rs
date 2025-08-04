use faber_core::{FaberError, Result};
use std::fs;
use std::path::Path;
use tracing::{info, warn};

/// Resource usage statistics from cgroup
#[derive(Debug, Clone)]
pub struct CgroupStats {
    pub memory_usage: u64,
    pub cpu_usage: u64,
    pub process_count: u32,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
}

/// Cgroup manager for resource limits
pub struct CgroupManager {
    pub prefix: String,
    pub base_path: Option<String>,
    pub cgroup_path: String,
}

impl CgroupManager {
    pub fn new(container_id: &str) -> std::result::Result<Self, super::error::SandboxError> {
        let prefix = "faber".to_string();
        let base_path = None;
        let cgroup_path = format!("{}/{}", prefix, container_id);

        // Create cgroup directory
        let cgroup_dir = Path::new("/sys/fs/cgroup").join(&cgroup_path);
        if let Err(e) = fs::create_dir_all(&cgroup_dir) {
            warn!("Failed to create cgroup directory: {}", e);
            // Continue without cgroups if not available
        }

        Ok(Self {
            prefix,
            base_path,
            cgroup_path,
        })
    }

    pub fn apply_limits(&self, limits: &super::container::ResourceLimits) -> Result<()> {
        info!("Applying resource limits to cgroup: {}", self.cgroup_path);

        // Set memory limit
        if let Err(e) = self.set_memory_limit(limits.memory_limit) {
            warn!("Failed to set memory limit: {}", e);
        }

        // Set CPU limit
        if let Some(cpu_rate) = limits.cpu_rate_limit {
            if let Err(e) = self.set_cpu_rate_limit(cpu_rate) {
                warn!("Failed to set CPU rate limit: {}", e);
            }
        }

        // Set process limit
        if let Err(e) = self.set_process_limit(limits.max_processes) {
            warn!("Failed to set process limit: {}", e);
        }

        Ok(())
    }

    pub fn add_process(&self, pid: u32) -> Result<()> {
        let tasks_file = format!("/sys/fs/cgroup/{}/tasks", self.cgroup_path);
        if let Err(e) = fs::write(&tasks_file, pid.to_string()) {
            warn!("Failed to add process {} to cgroup: {}", pid, e);
        }
        Ok(())
    }

    pub fn get_resource_stats(&self) -> Result<CgroupStats> {
        let memory_usage = self.read_memory_usage()?;
        let cpu_usage = self.read_cpu_usage()?;
        let process_count = self.read_process_count()?;
        let (io_read_bytes, io_write_bytes) = self.read_io_stats()?;

        Ok(CgroupStats {
            memory_usage,
            cpu_usage,
            process_count,
            io_read_bytes,
            io_write_bytes,
        })
    }

    pub fn cleanup(&self) -> Result<()> {
        info!("Cleaning up cgroup: {}", self.cgroup_path);

        let cgroup_dir = Path::new("/sys/fs/cgroup").join(&self.cgroup_path);
        if cgroup_dir.exists() {
            if let Err(e) = fs::remove_dir(&cgroup_dir) {
                warn!("Failed to remove cgroup directory: {}", e);
            }
        }

        Ok(())
    }

    fn set_memory_limit(&self, limit: u64) -> Result<()> {
        let memory_limit_file =
            format!("/sys/fs/cgroup/{}/memory.limit_in_bytes", self.cgroup_path);
        fs::write(&memory_limit_file, limit.to_string())
            .map_err(|e| FaberError::Sandbox(format!("Failed to set memory limit: {}", e)))
    }

    fn set_cpu_rate_limit(&self, rate: u32) -> Result<()> {
        let cpu_cfs_quota_file = format!("/sys/fs/cgroup/{}/cpu.cfs_quota_us", self.cgroup_path);
        let cpu_cfs_period_file = format!("/sys/fs/cgroup/{}/cpu.cfs_period_us", self.cgroup_path);

        // Set CPU period (default 100000 microseconds = 100ms)
        fs::write(&cpu_cfs_period_file, "100000")
            .map_err(|e| FaberError::Sandbox(format!("Failed to set CPU period: {}", e)))?;

        // Set CPU quota based on rate percentage
        let quota = (rate as u64 * 100000) / 100;
        fs::write(&cpu_cfs_quota_file, quota.to_string())
            .map_err(|e| FaberError::Sandbox(format!("Failed to set CPU quota: {}", e)))
    }

    fn set_process_limit(&self, limit: u32) -> Result<()> {
        let pids_max_file = format!("/sys/fs/cgroup/{}/pids.max", self.cgroup_path);
        fs::write(&pids_max_file, limit.to_string())
            .map_err(|e| FaberError::Sandbox(format!("Failed to set process limit: {}", e)))
    }

    fn read_memory_usage(&self) -> Result<u64> {
        let memory_usage_file =
            format!("/sys/fs/cgroup/{}/memory.usage_in_bytes", self.cgroup_path);
        let content = fs::read_to_string(&memory_usage_file)
            .map_err(|e| FaberError::Sandbox(format!("Failed to read memory usage: {}", e)))?;

        content
            .trim()
            .parse::<u64>()
            .map_err(|e| FaberError::Sandbox(format!("Failed to parse memory usage: {}", e)))
    }

    fn read_cpu_usage(&self) -> Result<u64> {
        let cpu_usage_file = format!("/sys/fs/cgroup/{}/cpuacct.usage", self.cgroup_path);
        let content = fs::read_to_string(&cpu_usage_file)
            .map_err(|e| FaberError::Sandbox(format!("Failed to read CPU usage: {}", e)))?;

        content
            .trim()
            .parse::<u64>()
            .map_err(|e| FaberError::Sandbox(format!("Failed to parse CPU usage: {}", e)))
    }

    fn read_process_count(&self) -> Result<u32> {
        let pids_current_file = format!("/sys/fs/cgroup/{}/pids.current", self.cgroup_path);
        let content = fs::read_to_string(&pids_current_file)
            .map_err(|e| FaberError::Sandbox(format!("Failed to read process count: {}", e)))?;

        content
            .trim()
            .parse::<u32>()
            .map_err(|e| FaberError::Sandbox(format!("Failed to parse process count: {}", e)))
    }

    fn read_io_stats(&self) -> Result<(u64, u64)> {
        let io_stat_file = format!(
            "/sys/fs/cgroup/{}/blkio.throttle.io_service_bytes",
            self.cgroup_path
        );
        let content = fs::read_to_string(&io_stat_file)
            .map_err(|e| FaberError::Sandbox(format!("Failed to read IO stats: {}", e)))?;

        let mut read_bytes = 0u64;
        let mut write_bytes = 0u64;

        for line in content.lines() {
            if line.contains("Read") {
                if let Some(value) = line.split_whitespace().last() {
                    read_bytes = value.parse::<u64>().unwrap_or(0);
                }
            } else if line.contains("Write") {
                if let Some(value) = line.split_whitespace().last() {
                    write_bytes = value.parse::<u64>().unwrap_or(0);
                }
            }
        }

        Ok((read_bytes, write_bytes))
    }
}

// Legacy CgroupsManager for backward compatibility
pub struct CgroupsManager {
    pub prefix: String,
    pub base_path: Option<String>,
}

impl CgroupsManager {
    pub fn new(prefix: String, base_path: Option<String>) -> Self {
        Self { prefix, base_path }
    }

    pub async fn create_cgroup(&self, name: &str) -> Result<()> {
        info!("Would create cgroup: {}/{}", self.prefix, name);
        // TODO: Implement actual cgroup creation
        Ok(())
    }

    pub async fn set_memory_limit(&self, name: &str, limit: u64) -> Result<()> {
        info!(
            "Would set memory limit for {}/{}: {} bytes",
            self.prefix, name, limit
        );
        // TODO: Implement memory limit setting
        Ok(())
    }

    pub async fn set_cpu_limit(&self, name: &str, limit: u32) -> Result<()> {
        info!(
            "Would set CPU limit for {}/{}: {}%",
            self.prefix, name, limit
        );
        // TODO: Implement CPU limit setting
        Ok(())
    }

    pub async fn cleanup_cgroup(&self, name: &str) -> Result<()> {
        info!("Would cleanup cgroup: {}/{}", self.prefix, name);
        // TODO: Implement cgroup cleanup
        Ok(())
    }
}
