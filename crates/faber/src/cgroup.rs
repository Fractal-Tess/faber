//! Cgroup v2 helpers for resource limiting and metrics collection.
//!
//! This module provides a simplified interface for cgroup v2 operations,
//! including controller enablement, limit setting, process attachment,
//! and metrics reading. It's designed to work with the existing Faber
//! runtime architecture.

use rand::Rng;
use std::fs::{create_dir_all, read_to_string, remove_dir, write};
use std::path::PathBuf;

use crate::prelude::*;
use crate::types::CgroupConfig;

/// Converts a human-readable memory string to bytes.
/// Supports formats like "128M", "1G", "512K", etc.
fn parse_memory_string(memory_str: &str) -> Result<u64> {
    let memory_str = memory_str.trim();

    if memory_str == "max" {
        return Ok(u64::MAX);
    }

    // Try to parse as a plain number first
    if let Ok(bytes) = memory_str.parse::<u64>() {
        return Ok(bytes);
    }

    // Parse with units
    let (number_str, unit) = memory_str.split_at(memory_str.len() - 1);
    let number: u64 = number_str.parse().map_err(|_| Error::Validation {
        field: "memory_max".to_string(),
        details: format!("Invalid memory format: {memory_str}"),
    })?;

    let multiplier = match unit.to_uppercase().as_str() {
        "K" => 1024,
        "M" => 1024 * 1024,
        "G" => 1024 * 1024 * 1024,
        "T" => 1024 * 1024 * 1024 * 1024,
        _ => {
            return Err(Error::Validation {
                field: "memory_max".to_string(),
                details: format!("Unknown memory unit: {unit}"),
            });
        }
    };

    Ok(number * multiplier)
}

/// CPU usage statistics from cgroup.
#[derive(Debug, Clone, Default)]
pub struct CpuStats {
    pub usage_usec: Option<u64>,
    pub user_usec: Option<u64>,
    pub system_usec: Option<u64>,
}

/// Memory usage statistics from cgroup.
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub current: u64,
    pub peak: u64,
    pub max: u64,
}

/// PIDs usage statistics from cgroup.
#[derive(Debug, Clone, Default)]
pub struct PidsStats {
    pub current: u64,
    pub max: u64,
}

/// Combined task statistics including CPU, memory, and PIDs usage.
#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    pub cpu: CpuStats,
    pub memory: MemoryStats,
    pub pids: PidsStats,
}

/// Creates the main faber cgroup hierarchy.
/// This should be called once per request in the parent process.
pub(crate) fn create_faber_cgroup_hierarchy() -> Result<()> {
    // Step 1: Write +cpu +memory +pids to /sys/fs/cgroup/cgroup.subtree_control
    write(
        "/sys/fs/cgroup/cgroup.subtree_control",
        "+cpu +memory +pids",
    )
    .map_err(|source| Error::WriteFile {
        path: PathBuf::from("/sys/fs/cgroup/cgroup.subtree_control"),
        bytes: "+cpu +memory +pids".len(),
        source,
        details: "Failed to write +cpu +memory +pids to cgroup.subtree_control".to_string(),
    })?;

    // Step 2: Create the folder /sys/fs/cgroup/faber
    let faber_cgroup_path = "/sys/fs/cgroup/faber";
    create_dir_all(faber_cgroup_path).map_err(|source| Error::CreateDir {
        path: PathBuf::from(faber_cgroup_path),
        source,
        details: "Failed to create faber cgroup directory".to_string(),
    })?;

    // Step 3: Write +cpu +memory +pids to /sys/fs/cgroup/faber/cgroup.subtree_control
    let faber_subtree_control = format!("{faber_cgroup_path}/cgroup.subtree_control");
    write(&faber_subtree_control, "+cpu +memory +pids").map_err(|source| Error::WriteFile {
        path: PathBuf::from(&faber_subtree_control),
        bytes: "+cpu +memory +pids".len(),
        source,
        details: "Failed to write +cpu +memory +pids to faber cgroup.subtree_control".to_string(),
    })?;

    Ok(())
}

/// Creates a task-specific cgroup within the faber hierarchy.
/// This should be called once per task in the parent process.
/// Returns the path to the created task directory for cleanup.
pub(crate) fn create_task_cgroup(config: &CgroupConfig) -> Result<String> {
    let faber_cgroup_path = "/sys/fs/cgroup/faber";

    // Step 4: Create the folder task-{random_16_characters}
    let task_id = generate_random_task_id();
    let task_cgroup_path = format!("{faber_cgroup_path}/task-{task_id}");
    create_dir_all(&task_cgroup_path).map_err(|source| Error::CreateDir {
        path: PathBuf::from(&task_cgroup_path),
        source,
        details: "Failed to create task cgroup directory".to_string(),
    })?;

    // Step 5: Write CPU limits to /sys/fs/cgroup/faber/task-{}/cpu.max
    let cpu_max_path = format!("{task_cgroup_path}/cpu.max");
    let cpu_max_value = config.cpu_max.as_deref().unwrap_or("50000 100000");
    write(&cpu_max_path, cpu_max_value).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&cpu_max_path),
        bytes: cpu_max_value.len(),
        source,
        details: format!("Failed to write CPU limits '{cpu_max_value}' to task cgroup"),
    })?;

    // Step 6: Write memory limit to /sys/fs/cgroup/faber/task-{}/memory.max
    let memory_max_path = format!("{task_cgroup_path}/memory.max");
    let memory_max_value = config.memory_max.as_deref().unwrap_or("134217728"); // 128MB default

    // Convert human-readable memory string to bytes
    let memory_max_bytes = parse_memory_string(memory_max_value)?;

    write(&memory_max_path, memory_max_bytes.to_string()).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&memory_max_path),
        bytes: memory_max_bytes.to_string().len(),
        source,
        details: format!("Failed to write memory limit '{memory_max_bytes}' bytes to task cgroup"),
    })?;

    // Step 7: Write PIDs limit to /sys/fs/cgroup/faber/task-{}/pids.max
    let pids_max_path = format!("{task_cgroup_path}/pids.max");
    let pids_max_value = config
        .pids_max
        .map(|p| p.to_string())
        .unwrap_or_else(|| "64".to_string());
    write(&pids_max_path, &pids_max_value).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&pids_max_path),
        bytes: pids_max_value.len(),
        source,
        details: format!("Failed to write PIDs limit '{pids_max_value}' to task cgroup"),
    })?;

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
    // Remove the task directory and all its contents
    remove_dir(task_cgroup_path).map_err(|source| Error::RemoveDir {
        path: PathBuf::from(task_cgroup_path),
        source,
        details: "Failed to remove task cgroup directory".to_string(),
    })?;

    Ok(())
}

/// Adds a process to the task cgroup by writing its PID to cgroup.procs.
pub(crate) fn add_process_to_task_cgroup(task_cgroup_path: &str, pid: u32) -> Result<()> {
    let cgroup_procs_path = format!("{task_cgroup_path}/cgroup.procs");
    let pid_str = pid.to_string();

    write(&cgroup_procs_path, &pid_str).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&cgroup_procs_path),
        bytes: pid_str.len(),
        source,
        details: format!("Failed to add process {pid} to task cgroup"),
    })?;

    Ok(())
}

/// Reads and parses CPU, memory, and PIDs statistics from a task cgroup after task completion.
/// Returns parsed task statistics that can be attached to TaskResult.
pub(crate) fn read_task_stats(task_cgroup_path: &str) -> Result<TaskStats> {
    let mut cpu_stats = CpuStats::default();
    let mut memory_stats = MemoryStats::default();
    let mut pids_stats = PidsStats::default();

    // Read cpu.stat file which contains CPU usage statistics
    let cpu_stat_path = format!("{task_cgroup_path}/cpu.stat");

    match read_to_string(&cpu_stat_path) {
        Ok(contents) => {
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
        }
        Err(e) => {}
    }

    // Read memory statistics

    // Read current memory usage
    let memory_current_path = format!("{task_cgroup_path}/memory.current");
    match read_to_string(&memory_current_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                memory_stats.current = value;
            }
        }
        Err(e) => {}
    }

    // Read peak memory usage
    let memory_peak_path = format!("{task_cgroup_path}/memory.peak");
    match read_to_string(&memory_peak_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                memory_stats.peak = value;
            }
        }
        Err(e) => {}
    }

    // Read memory limit (max)
    let memory_max_path = format!("{task_cgroup_path}/memory.max");
    match read_to_string(&memory_max_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                memory_stats.max = value;
            }
        }
        Err(e) => {}
    }

    // Read PIDs statistics

    // Read current PIDs count
    let pids_current_path = format!("{task_cgroup_path}/pids.current");
    match read_to_string(&pids_current_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                pids_stats.current = value;
            }
        }
        Err(e) => {}
    }

    // Read PIDs limit (max)
    let pids_max_path = format!("{task_cgroup_path}/pids.max");
    match read_to_string(&pids_max_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                pids_stats.max = value;
            }
        }
        Err(e) => {}
    }

    // Read cpu.max file to show the limits that were set (for debugging)
    let cpu_max_path = format!("{task_cgroup_path}/cpu.max");

    // Read cgroup.procs to show which processes were in this cgroup (for debugging)
    let cgroup_procs_path = format!("{task_cgroup_path}/cgroup.procs");

    Ok(TaskStats {
        cpu: cpu_stats,
        memory: memory_stats,
        pids: pids_stats,
    })
}
