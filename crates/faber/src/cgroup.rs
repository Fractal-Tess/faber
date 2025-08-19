//! Cgroup v2 helpers for resource limiting and metrics collection.
//!
//! This module provides a simplified interface for cgroup v2 operations,
//! including controller enablement, limit setting, process attachment,
//! and metrics reading. It's designed to work with the existing Faber
//! runtime architecture.

use rand::Rng;
use std::fs::{create_dir_all, read_to_string, remove_dir_all, write};
use std::path::PathBuf;

use crate::prelude::*;
use crate::types::CgroupConfig;

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
    eprintln!("[CGROUP] Starting to create main faber cgroup hierarchy");

    // Step 1: Write +cpu +memory +pids to /sys/fs/cgroup/cgroup.subtree_control
    eprintln!(
        "[CGROUP] Step 1: Writing +cpu +memory +pids to /sys/fs/cgroup/cgroup.subtree_control"
    );
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

    // Step 3: Write +cpu +memory +pids to /sys/fs/cgroup/faber/cgroup.subtree_control
    eprintln!(
        "[CGROUP] Step 3: Writing +cpu +memory +pids to /sys/fs/cgroup/faber/cgroup.subtree_control"
    );
    let faber_subtree_control = format!("{faber_cgroup_path}/cgroup.subtree_control");
    write(&faber_subtree_control, "+cpu +memory +pids").map_err(|source| Error::WriteFile {
        path: PathBuf::from(&faber_subtree_control),
        bytes: "+cpu +memory +pids".len(),
        source,
        details: "Failed to write +cpu +memory +pids to faber cgroup.subtree_control".to_string(),
    })?;
    debug_read_file(&faber_subtree_control)?;

    eprintln!("[CGROUP] Successfully completed main faber cgroup setup");
    Ok(())
}

/// Creates a task-specific cgroup within the faber hierarchy.
/// This should be called once per task in the parent process.
/// Returns the path to the created task directory for cleanup.
pub(crate) fn create_task_cgroup(config: &CgroupConfig) -> Result<String> {
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

    // Step 5: Write CPU limits to /sys/fs/cgroup/faber/task-{}/cpu.max
    eprintln!("[CGROUP] Step 5: Writing CPU limits to task cgroup");
    let cpu_max_path = format!("{task_cgroup_path}/cpu.max");
    let cpu_max_value = config.cpu_max.as_deref().unwrap_or("50000 100000");
    write(&cpu_max_path, cpu_max_value).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&cpu_max_path),
        bytes: cpu_max_value.len(),
        source,
        details: format!(
            "Failed to write CPU limits '{}' to task cgroup",
            cpu_max_value
        ),
    })?;
    debug_read_file(&cpu_max_path)?;

    // Step 6: Write memory limit to /sys/fs/cgroup/faber/task-{}/memory.max
    eprintln!("[CGROUP] Step 6: Writing memory limit to task cgroup");
    let memory_max_path = format!("{task_cgroup_path}/memory.max");
    let memory_max_value = config.memory_max.as_deref().unwrap_or("134217728"); // 128MB default
    write(&memory_max_path, memory_max_value).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&memory_max_path),
        bytes: memory_max_value.len(),
        source,
        details: format!(
            "Failed to write memory limit '{}' to task cgroup",
            memory_max_value
        ),
    })?;
    debug_read_file(&memory_max_path)?;

    // Step 7: Write PIDs limit to /sys/fs/cgroup/faber/task-{}/pids.max
    eprintln!("[CGROUP] Step 7: Writing PIDs limit to task cgroup");
    let pids_max_path = format!("{task_cgroup_path}/pids.max");
    let pids_max_value = config
        .pids_max
        .map(|p| p.to_string())
        .unwrap_or_else(|| "64".to_string());
    write(&pids_max_path, &pids_max_value).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&pids_max_path),
        bytes: pids_max_value.len(),
        source,
        details: format!(
            "Failed to write PIDs limit '{}' to task cgroup",
            pids_max_value
        ),
    })?;
    debug_read_file(&pids_max_path)?;

    eprintln!("[CGROUP] Successfully created task cgroup: {task_cgroup_path}");
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
    eprintln!("[CGROUP] Cleaning up task cgroup directory: {task_cgroup_path}");

    // Remove the task directory and all its contents
    remove_dir_all(task_cgroup_path).map_err(|source| Error::RemoveDir {
        path: PathBuf::from(task_cgroup_path),
        source,
        details: "Failed to remove task cgroup directory".to_string(),
    })?;

    eprintln!("[CGROUP] Successfully cleaned up task cgroup directory: {task_cgroup_path}");
    Ok(())
}

/// Adds a process to the task cgroup by writing its PID to cgroup.procs.
pub(crate) fn add_process_to_task_cgroup(task_cgroup_path: &str, pid: u32) -> Result<()> {
    let cgroup_procs_path = format!("{task_cgroup_path}/cgroup.procs");
    let pid_str = pid.to_string();

    eprintln!("[CGROUP] Adding process {pid} to task cgroup: {task_cgroup_path}");

    write(&cgroup_procs_path, &pid_str).map_err(|source| Error::WriteFile {
        path: PathBuf::from(&cgroup_procs_path),
        bytes: pid_str.len(),
        source,
        details: format!("Failed to add process {pid} to task cgroup"),
    })?;

    eprintln!("[CGROUP] Successfully added process {pid} to task cgroup");
    debug_read_file(&cgroup_procs_path)?;

    Ok(())
}

/// Reads and parses CPU, memory, and PIDs statistics from a task cgroup after task completion.
/// Returns parsed task statistics that can be attached to TaskResult.
pub(crate) fn read_task_stats(task_cgroup_path: &str) -> Result<TaskStats> {
    eprintln!("[CGROUP] Reading task statistics for task cgroup: {task_cgroup_path}");

    let mut cpu_stats = CpuStats::default();
    let mut memory_stats = MemoryStats::default();
    let mut pids_stats = PidsStats::default();

    // Read cpu.stat file which contains CPU usage statistics
    let cpu_stat_path = format!("{task_cgroup_path}/cpu.stat");
    eprintln!("[CGROUP] Reading CPU stats from: {cpu_stat_path}");

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
            eprintln!("[DEBUG] Failed to read cpu.stat file {cpu_stat_path}: {e}");
        }
    }

    // Read memory statistics
    eprintln!("[CGROUP] Reading memory stats from task cgroup");

    // Read current memory usage
    let memory_current_path = format!("{task_cgroup_path}/memory.current");
    match read_to_string(&memory_current_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                memory_stats.current = value;
                eprintln!(
                    "[CGROUP] Current memory usage: {value} bytes ({:.2} MB)",
                    value as f64 / (1024.0 * 1024.0)
                );
            }
        }
        Err(e) => {
            eprintln!("[DEBUG] Failed to read memory.current file {memory_current_path}: {e}");
        }
    }

    // Read peak memory usage
    let memory_peak_path = format!("{task_cgroup_path}/memory.peak");
    match read_to_string(&memory_peak_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                memory_stats.peak = value;
                eprintln!(
                    "[CGROUP] Peak memory usage: {value} bytes ({:.2} MB)",
                    value as f64 / (1024.0 * 1024.0)
                );
            }
        }
        Err(e) => {
            eprintln!("[DEBUG] Failed to read memory.peak file {memory_peak_path}: {e}");
        }
    }

    // Read memory limit (max)
    let memory_max_path = format!("{task_cgroup_path}/memory.max");
    match read_to_string(&memory_max_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                memory_stats.max = value;
                eprintln!(
                    "[CGROUP] Memory limit: {value} bytes ({:.2} MB)",
                    value as f64 / (1024.0 * 1024.0)
                );
            }
        }
        Err(e) => {
            eprintln!("[DEBUG] Failed to read memory.max file {memory_max_path}: {e}");
        }
    }

    // Read PIDs statistics
    eprintln!("[CGROUP] Reading PIDs stats from task cgroup");

    // Read current PIDs count
    let pids_current_path = format!("{task_cgroup_path}/pids.current");
    match read_to_string(&pids_current_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                pids_stats.current = value;
                eprintln!("[CGROUP] Current PIDs count: {value}");
            }
        }
        Err(e) => {
            eprintln!("[DEBUG] Failed to read pids.current file {pids_current_path}: {e}");
        }
    }

    // Read PIDs limit (max)
    let pids_max_path = format!("{task_cgroup_path}/pids.max");
    match read_to_string(&pids_max_path) {
        Ok(content) => {
            if let Ok(value) = content.trim().parse::<u64>() {
                pids_stats.max = value;
                eprintln!("[CGROUP] PIDs limit: {value}");
            }
        }
        Err(e) => {
            eprintln!("[DEBUG] Failed to read pids.max file {pids_max_path}: {e}");
        }
    }

    // Read cpu.max file to show the limits that were set (for debugging)
    let cpu_max_path = format!("{task_cgroup_path}/cpu.max");
    eprintln!("[CGROUP] Reading CPU limits from: {cpu_max_path}");
    debug_read_file(&cpu_max_path)?;

    // Read cgroup.procs to show which processes were in this cgroup (for debugging)
    let cgroup_procs_path = format!("{task_cgroup_path}/cgroup.procs");
    eprintln!("[CGROUP] Reading processes from: {cgroup_procs_path}");
    debug_read_file(&cgroup_procs_path)?;

    eprintln!("[CGROUP] Completed reading task statistics for task cgroup");
    Ok(TaskStats {
        cpu: cpu_stats,
        memory: memory_stats,
        pids: pids_stats,
    })
}

/// Backward compatibility function for existing code.
/// This function is deprecated and will be removed in a future version.
/// Use `read_task_stats` instead.
#[deprecated(since = "0.2.0", note = "Use read_task_stats instead")]
pub(crate) fn read_task_cpu_stats(task_cgroup_path: &str) -> Result<CpuStats> {
    read_task_stats(task_cgroup_path).map(|stats| stats.cpu)
}

/// Reads and prints the contents of a file for debugging purposes.
pub(crate) fn debug_read_file(path: &str) -> Result<()> {
    eprintln!("[DEBUG] Reading contents of file: {path}");

    match read_to_string(path) {
        Ok(contents) => {
            eprintln!("[DEBUG] File contents: '{}'", contents.trim());
        }
        Err(e) => {
            eprintln!("[DEBUG] Failed to read file {path}: {e}");
        }
    }

    Ok(())
}

/// Lists all files and directories at the specified path for debugging purposes.
pub(crate) fn debug_list_files(path: &str) -> Result<()> {
    eprintln!("[DEBUG] Listing contents of directory: {path}");

    match std::fs::read_dir(path) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let file_name = entry.file_name().to_string_lossy().to_string();
                        match entry.metadata() {
                            Ok(meta) => {
                                if meta.is_dir() {
                                    eprintln!("[DEBUG]   DIR:  {file_name}");
                                } else if meta.is_file() {
                                    eprintln!("[DEBUG]   FILE: {file_name} ({} bytes)", meta.len());
                                } else {
                                    eprintln!("[DEBUG]   OTHER: {file_name}");
                                }
                            }
                            Err(e) => {
                                eprintln!("[DEBUG]   ERROR reading metadata for {file_name}: {e}")
                            }
                        }
                    }
                    Err(e) => eprintln!("[DEBUG]   ERROR reading directory entry: {e}"),
                }
            }
        }
        Err(e) => eprintln!("[DEBUG] Failed to read directory {path}: {e}"),
    }

    Ok(())
}
