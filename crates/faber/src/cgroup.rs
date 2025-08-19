//! Cgroup v2 helpers for resource limiting and metrics collection.
//!
//! This module provides a simplified interface for cgroup v2 operations,
//! including controller enablement, limit setting, process attachment,
//! and metrics reading. It's designed to work with the existing Faber
//! runtime architecture.

use rand::Rng;
use std::fs::{self, create_dir_all, read_to_string, remove_dir_all, write};
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
    pub usage_usec: Option<u64>,
    pub user_usec: Option<u64>,
    pub system_usec: Option<u64>,
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
                        "usage_usec" => stats.usage_usec = Some(value),
                        "user_usec" => stats.user_usec = Some(value),
                        "system_usec" => stats.system_usec = Some(value),
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

// ===== Functions moved from environment.rs =====

/// Creates the main faber cgroup hierarchy.
/// This should be called once per request in the parent process.
pub(crate) fn create_faber_cgroup_hierarchy() -> Result<()> {
    eprintln!("[CGROUP] Starting to create main faber cgroup hierarchy");

    // Step 1: Write +cpu to /sys/fs/cgroup/cgroup.subtree_control
    eprintln!("[CGROUP] Step 1: Writing +cpu to /sys/fs/cgroup/cgroup.subtree_control");
    write("/sys/fs/cgroup/cgroup.subtree_control", "+cpu").map_err(|source| Error::WriteFile {
        path: PathBuf::from("/sys/fs/cgroup/cgroup.subtree_control"),
        bytes: "+cpu".len(),
        source,
        details: "Failed to write +cpu to cgroup.subtree_control".to_string(),
    })?;
    debug_read_file("/sys/fs/cgroup/cgroup.subtree_control")?;

    // Step 2: Create the folder /sys/fs/cgroup/faber
    eprintln!("[CGROUP] Step 2: Creating /sys/fs/cgroup/faber directory");
    let faber_cgroup_path = "/sys/fs/cgroup/faber";
    create_dir_all(faber_cgroup_path).map_err(|source| Error::CreateDir {
        path: PathBuf::from(faber_cgroup_path),
        source,
        details: "Failed to create faber cgroup directory".to_string(),
    })?;
    debug_list_files("/sys/fs/cgroup/faber")?;

    // Step 3: Write +cpu to /sys/fs/cgroup/faber/cgroup.subtree_control
    eprintln!("[CGROUP] Step 3: Writing +cpu to /sys/fs/cgroup/faber/cgroup.subtree_control");
    let faber_subtree_control = format!("{faber_cgroup_path}/cgroup.subtree_control");
    write(&faber_subtree_control, "+cpu").map_err(|source| Error::WriteFile {
        path: PathBuf::from(&faber_subtree_control),
        bytes: "+cpu".len(),
        source,
        details: "Failed to write +cpu to faber cgroup.subtree_control".to_string(),
    })?;
    debug_read_file(&faber_subtree_control)?;

    eprintln!("[CGROUP] Successfully completed main faber cgroup setup");
    Ok(())
}

/// Creates a task-specific cgroup within the faber hierarchy.
/// This should be called once per task in the parent process.
/// Returns the path to the created task directory for cleanup.
pub(crate) fn create_task_cgroup() -> Result<String> {
    eprintln!("[CGROUP] Creating task-specific cgroup");

    let faber_cgroup_path = "/sys/fs/cgroup/faber";

    // Step 4: Create the folder task-{random_16_characters}
    eprintln!("[CGROUP] Step 4: Creating task directory with random name");
    let task_id = generate_random_task_id();
    let task_cgroup_path = format!("{faber_cgroup_path}/task-{task_id}");
    create_dir_all(&task_cgroup_path).map_err(|source| Error::CreateDir {
        path: PathBuf::from(&task_cgroup_path),
        source,
        details: "Failed to create task cgroup directory".to_string(),
    })?;
    debug_list_files(&task_cgroup_path)?;

    // Step 5: Write 50000 100000 to /sys/fs/cgroup/faber/task-{}/cpu.max
    eprintln!("[CGROUP] Step 5: Writing CPU limits to task cgroup");
    let cpu_max_path = format!("{task_cgroup_path}/cpu.max");
    write(&cpu_max_path, "50000 100000").map_err(|source| Error::WriteFile {
        path: PathBuf::from(&cpu_max_path),
        bytes: "50000 100000".len(),
        source,
        details: "Failed to write CPU limits to task cgroup".to_string(),
    })?;
    debug_read_file(&cpu_max_path)?;

    eprintln!(
        "[CGROUP] Successfully created task cgroup: {}",
        task_cgroup_path
    );
    Ok(task_cgroup_path)
}

/// Generates a random 16-character task ID using alphanumeric characters.
fn generate_random_task_id() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();

    (0..16)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Removes a task cgroup directory and its contents.
pub(crate) fn cleanup_task_cgroup(task_cgroup_path: &str) -> Result<()> {
    eprintln!(
        "[CGROUP] Cleaning up task cgroup directory: {}",
        task_cgroup_path
    );

    // Remove the task directory and all its contents
    remove_dir_all(task_cgroup_path).map_err(|source| Error::RemoveDir {
        path: PathBuf::from(task_cgroup_path),
        source,
        details: "Failed to remove task cgroup directory".to_string(),
    })?;

    eprintln!(
        "[CGROUP] Successfully cleaned up task cgroup directory: {}",
        task_cgroup_path
    );
    Ok(())
}

/// Removes the main faber cgroup directory and all its contents.
pub(crate) fn cleanup_faber_cgroup() -> Result<()> {
    let faber_cgroup_path = "/sys/fs/cgroup/faber";
    eprintln!(
        "[CGROUP] Cleaning up main faber cgroup directory: {}",
        faber_cgroup_path
    );

    // Remove the faber cgroup directory and all its contents
    remove_dir_all(faber_cgroup_path).map_err(|source| Error::RemoveDir {
        path: PathBuf::from(faber_cgroup_path),
        source,
        details: "Failed to remove main faber cgroup directory".to_string(),
    })?;

    eprintln!(
        "[CGROUP] Successfully cleaned up main faber cgroup directory: {}",
        faber_cgroup_path
    );
    Ok(())
}

/// Adds a process to the task cgroup by writing its PID to cgroup.procs.
pub(crate) fn add_process_to_task_cgroup(task_cgroup_path: &str, pid: u32) -> Result<()> {
    let cgroup_procs_path = format!("{}/cgroup.procs", task_cgroup_path);
    let pid_str = pid.to_string();

    eprintln!(
        "[CGROUP] Adding process {} to task cgroup: {}",
        pid, task_cgroup_path
    );

    write(&cgroup_procs_path, &pid_str).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&cgroup_procs_path),
        bytes: pid_str.len(),
        source,
        details: format!("Failed to add process {} to task cgroup", pid),
    })?;

    eprintln!("[CGROUP] Successfully added process {} to task cgroup", pid);
    debug_read_file(&cgroup_procs_path)?;

    Ok(())
}

/// Reads and parses CPU statistics from a task cgroup after task completion.
/// Returns parsed CPU statistics that can be attached to TaskResult.
pub(crate) fn read_task_cpu_stats(task_cgroup_path: &str) -> Result<CpuStats> {
    eprintln!(
        "[CGROUP] Reading CPU statistics for task cgroup: {}",
        task_cgroup_path
    );

    let mut cpu_stats = CpuStats::default();

    // Read cpu.stat file which contains CPU usage statistics
    let cpu_stat_path = format!("{}/cpu.stat", task_cgroup_path);
    eprintln!("[CGROUP] Reading CPU stats from: {}", cpu_stat_path);

    match read_to_string(&cpu_stat_path) {
        Ok(contents) => {
            eprintln!("[DEBUG] cpu.stat contents: '{}'", contents.trim());

            // Parse cpu.stat file format:
            // usage_usec 1234567
            // user_usec 987654
            // system_usec 246913
            for line in contents.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2 {
                    if let Ok(value) = parts[1].parse::<u64>() {
                        match parts[0] {
                            "usage_usec" => cpu_stats.usage_usec = Some(value),
                            "user_usec" => cpu_stats.user_usec = Some(value),
                            "system_usec" => cpu_stats.system_usec = Some(value),
                            _ => {} // Ignore other fields
                        }
                    }
                }
            }

            eprintln!(
                "[CGROUP] Parsed CPU stats - usage: {:?}, user: {:?}, system: {:?}",
                cpu_stats.usage_usec, cpu_stats.user_usec, cpu_stats.system_usec
            );
        }
        Err(e) => {
            eprintln!(
                "[DEBUG] Failed to read cpu.stat file {}: {}",
                cpu_stat_path, e
            );
        }
    }

    // Read cpu.max file to show the limits that were set (for debugging)
    let cpu_max_path = format!("{}/cpu.max", task_cgroup_path);
    eprintln!("[CGROUP] Reading CPU limits from: {}", cpu_max_path);
    debug_read_file(&cpu_max_path)?;

    // Read cgroup.procs to show which processes were in this cgroup (for debugging)
    let cgroup_procs_path = format!("{}/cgroup.procs", task_cgroup_path);
    eprintln!("[CGROUP] Reading processes from: {}", cgroup_procs_path);
    debug_read_file(&cgroup_procs_path)?;

    eprintln!("[CGROUP] Completed reading CPU statistics for task cgroup");
    Ok(cpu_stats)
}

/// Reads and prints the contents of a file for debugging purposes.
pub(crate) fn debug_read_file(path: &str) -> Result<()> {
    eprintln!("[DEBUG] Reading contents of file: {}", path);

    match read_to_string(path) {
        Ok(contents) => {
            eprintln!("[DEBUG] File contents: '{}'", contents.trim());
        }
        Err(e) => {
            eprintln!("[DEBUG] Failed to read file {}: {}", path, e);
        }
    }

    Ok(())
}

/// Lists all files and directories at the specified path for debugging purposes.
pub(crate) fn debug_list_files(path: &str) -> Result<()> {
    eprintln!("[DEBUG] Listing contents of directory: {}", path);

    match std::fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        match entry.metadata() {
                            Ok(meta) => {
                                if meta.is_dir() {
                                    eprintln!("[DEBUG]   DIR:  {}", file_name);
                                } else if meta.is_file() {
                                    eprintln!(
                                        "[DEBUG]   FILE: {} ({} bytes)",
                                        file_name,
                                        meta.len()
                                    );
                                } else {
                                    eprintln!("[DEBUG]   OTHER: {}", file_name);
                                }
                            }
                            Err(e) => eprintln!(
                                "[DEBUG]   ERROR reading metadata for {}: {}",
                                file_name, e
                            ),
                        }
                    }
                    Err(e) => eprintln!("[DEBUG]   ERROR reading directory entry: {}", e),
                }
            }
        }
        Err(e) => eprintln!("[DEBUG] Failed to read directory {}: {}", path, e),
    }

    Ok(())
}
