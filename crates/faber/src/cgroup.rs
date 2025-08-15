//! Cgroup v2 helpers for resource limiting and metrics collection.
//!
//! This module provides a simplified interface for cgroup v2 operations,
//! including controller enablement, limit setting, process attachment,
//! and metrics reading. It's designed to work with the existing Faber
//! runtime architecture.

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::prelude::*;

/// Cgroup controller types that can be enabled.
#[derive(Debug, Clone, Copy)]
pub enum Controller {
    Memory,
    Pids,
    Cpu,
}

impl Controller {
    fn as_str(&self) -> &'static str {
        match self {
            Controller::Memory => "memory",
            Controller::Pids => "pids",
            Controller::Cpu => "cpu",
        }
    }
}

/// CPU usage statistics from cgroup.
#[derive(Debug, Clone, Default)]
pub struct CpuStats {
    pub usage_usec: u64,
    pub user_usec: u64,
    pub system_usec: u64,
}

/// Memory statistics from cgroup.
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub current: u64,
    pub peak: u64,
}

/// Cgroup manager for a specific cgroup path.
#[derive(Debug)]
pub struct CgroupManager {
    path: PathBuf,
}

impl CgroupManager {
    /// Create a new cgroup manager for the given path.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Get the cgroup path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Enable controllers in the current cgroup.
    /// This writes to `cgroup.subtree_control` to enable controllers for child cgroups.
    pub fn enable_controllers(&self, controllers: &[Controller]) -> Result<()> {
        let subtree_control_path = self.path.join("cgroup.subtree_control");

        let mut content = String::new();
        for controller in controllers {
            content.push_str(&format!("+{} ", controller.as_str()));
        }
        let content = content.trim().to_string();

        fs::write(&subtree_control_path, &content).map_err(|source| Error::Cgroup {
            message: "Failed to enable controllers".to_string(),
            details: format!(
                "Failed to write '{}' to {}: {}",
                content,
                subtree_control_path.display(),
                source
            ),
        })?;

        Ok(())
    }

    /// Create a child cgroup under this cgroup.
    pub fn create_child(&self, name: &str) -> Result<CgroupManager> {
        let child_path = self.path.join(name);
        fs::create_dir(&child_path).map_err(|source| Error::Cgroup {
            message: "Failed to create child cgroup".to_string(),
            details: format!(
                "Failed to create directory {}: {}",
                child_path.display(),
                source
            ),
        })?;

        Ok(CgroupManager::new(child_path))
    }

    /// Set memory limit in bytes.
    pub fn set_memory_max(&self, bytes: u64) -> Result<()> {
        let memory_max_path = self.path.join("memory.max");
        fs::write(&memory_max_path, bytes.to_string()).map_err(|source| Error::Cgroup {
            message: "Failed to set memory limit".to_string(),
            details: format!(
                "Failed to write {} to {}: {}",
                bytes,
                memory_max_path.display(),
                source
            ),
        })?;

        // Also set swap.max to 0 to prevent swapping
        let swap_max_path = self.path.join("memory.swap.max");
        fs::write(&swap_max_path, "0").map_err(|source| Error::Cgroup {
            message: "Failed to set swap limit".to_string(),
            details: format!(
                "Failed to write 0 to {}: {}",
                swap_max_path.display(),
                source
            ),
        })?;

        Ok(())
    }

    /// Set process count limit.
    pub fn set_pids_max(&self, max: u64) -> Result<()> {
        let pids_max_path = self.path.join("pids.max");
        fs::write(&pids_max_path, max.to_string()).map_err(|source| Error::Cgroup {
            message: "Failed to set pids limit".to_string(),
            details: format!(
                "Failed to write {} to {}: {}",
                max,
                pids_max_path.display(),
                source
            ),
        })?;

        Ok(())
    }

    /// Set CPU limit.
    /// Format: "max" for unlimited, or "quota period" (e.g., "20000 100000" for 20% CPU).
    pub fn set_cpu_max(&self, spec: &str) -> Result<()> {
        let cpu_max_path = self.path.join("cpu.max");
        fs::write(&cpu_max_path, spec).map_err(|source| Error::Cgroup {
            message: "Failed to set cpu limit".to_string(),
            details: format!(
                "Failed to write '{}' to {}: {}",
                spec,
                cpu_max_path.display(),
                source
            ),
        })?;

        Ok(())
    }

    /// Add a process to this cgroup by writing its PID to cgroup.procs.
    pub fn add_proc(&self, pid: u32) -> Result<()> {
        let procs_path = self.path.join("cgroup.procs");
        fs::write(&procs_path, pid.to_string()).map_err(|source| Error::Cgroup {
            message: "Failed to add process to cgroup".to_string(),
            details: format!(
                "Failed to write PID {} to {}: {}",
                pid,
                procs_path.display(),
                source
            ),
        })?;

        Ok(())
    }

    /// Read CPU statistics from this cgroup.
    pub fn read_cpu_stats(&self) -> Result<CpuStats> {
        let cpu_stat_path = self.path.join("cpu.stat");
        let content = fs::read_to_string(&cpu_stat_path).map_err(|source| Error::Cgroup {
            message: "Failed to read cpu.stat".to_string(),
            details: format!("Failed to read {}: {}", cpu_stat_path.display(), source),
        })?;

        let mut stats = CpuStats::default();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(value) = u64::from_str(parts[1]) {
                    match parts[0] {
                        "usage_usec" => stats.usage_usec = value,
                        "user_usec" => stats.user_usec = value,
                        "system_usec" => stats.system_usec = value,
                        _ => {}
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Read memory statistics from this cgroup.
    pub fn read_memory_stats(&self) -> Result<MemoryStats> {
        let mut stats = MemoryStats::default();

        // Read current memory usage
        let current_path = self.path.join("memory.current");
        if let Ok(content) = fs::read_to_string(&current_path) {
            if let Ok(value) = u64::from_str(content.trim()) {
                stats.current = value;
            }
        }

        // Read peak memory usage
        let peak_path = self.path.join("memory.peak");
        if let Ok(content) = fs::read_to_string(&peak_path) {
            if let Ok(value) = u64::from_str(content.trim()) {
                stats.peak = value;
            }
        }

        Ok(stats)
    }
}

/// Create a cgroup at the root level with the given name.
pub fn create_root_cgroup(name: &str) -> Result<CgroupManager> {
    let root_path = Path::new("/sys/fs/cgroup");
    let cgroup_path = root_path.join(name);

    fs::create_dir(&cgroup_path).map_err(|source| Error::Cgroup {
        message: "Failed to create root cgroup".to_string(),
        details: format!(
            "Failed to create directory {}: {}",
            cgroup_path.display(),
            source
        ),
    })?;

    Ok(CgroupManager::new(cgroup_path))
}

/// Parse memory size string (e.g., "256M", "1G") to bytes.
pub fn parse_memory_size(size_str: &str) -> Result<u64> {
    let size_str = size_str.trim();

    if size_str == "max" {
        return Ok(u64::MAX);
    }

    let (number_str, unit) = if size_str.ends_with('K') || size_str.ends_with('k') {
        (&size_str[..size_str.len() - 1], 1024)
    } else if size_str.ends_with('M') || size_str.ends_with('m') {
        (&size_str[..size_str.len() - 1], 1024 * 1024)
    } else if size_str.ends_with('G') || size_str.ends_with('g') {
        (&size_str[..size_str.len() - 1], 1024 * 1024 * 1024)
    } else {
        (size_str, 1)
    };

    let number = u64::from_str(number_str).map_err(|_| Error::Cgroup {
        message: "Failed to parse memory size".to_string(),
        details: format!("Invalid number in size string: {}", number_str),
    })?;

    Ok(number * unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_size() {
        assert_eq!(parse_memory_size("1024").unwrap(), 1024);
        assert_eq!(parse_memory_size("1K").unwrap(), 1024);
        assert_eq!(parse_memory_size("1M").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_size("1G").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_size("max").unwrap(), u64::MAX);
        assert_eq!(parse_memory_size("256M").unwrap(), 256 * 1024 * 1024);
    }

    #[test]
    fn test_controller_as_str() {
        assert_eq!(Controller::Memory.as_str(), "memory");
        assert_eq!(Controller::Pids.as_str(), "pids");
        assert_eq!(Controller::Cpu.as_str(), "cpu");
    }

    #[test]
    fn test_cgroup_manager_creation() {
        let manager = CgroupManager::new(PathBuf::from("/tmp/test"));
        assert_eq!(manager.path(), Path::new("/tmp/test"));
    }
}
