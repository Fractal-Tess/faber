use std::fs::{create_dir_all, read_to_string, remove_dir, write};
use std::path::{Path, PathBuf};

use rand::rand_core::le;

use crate::prelude::*;
use crate::task::CgroupConfig;
use crate::utils::generate_random_string;
fn parse_memory_string(memory_str: &str) -> Result<u64> {
    let memory_str = memory_str.trim();

    if memory_str == "max" {
        return Ok(u64::MAX);
    }

    if let Ok(bytes) = memory_str.parse::<u64>() {
        return Ok(bytes);
    }
    let (number_str, unit) = memory_str.split_at(memory_str.len() - 1);
    let number: u64 = number_str.parse().map_err(|_| FaberError::Generic {
        message: format!("Invalid memory format: {}", memory_str),
    })?;

    let multiplier = match unit.to_uppercase().as_str() {
        "K" => 1024,
        "M" => 1024 * 1024,
        "G" => 1024 * 1024 * 1024,
        "T" => 1024 * 1024 * 1024 * 1024,
        _ => {
            return Err(FaberError::Generic {
                message: format!("Unknown memory unit: {}", unit),
            });
        }
    };

    Ok(number * multiplier)
}

#[derive(Debug, Clone, Default)]
pub struct CpuStats {
    pub usage_usec: u64,
    pub user_usec: u64,
    pub system_usec: u64,
}

#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub current: u64,
    pub peak: u64,
    pub max: u64,
}

#[derive(Debug, Clone, Default)]
pub struct PidsStats {
    pub current: u64,
    pub max: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    pub cpu: CpuStats,
    pub memory: MemoryStats,
    pub pids: PidsStats,
}

fn list_files_in_directory(directory: &str) -> () {
    use std::fs;

    // List all files and directories under /sys/fs/cgroup, sorted by name
    let mut entries: Vec<_> = fs::read_dir(directory)
        .map_err(|e| FaberError::Generic {
            message: format!("Failed to list {}: {}", directory, e),
        })
        .expect(&format!("Failed to list {}", directory))
        .collect();

    // Sort entries by file name
    entries.sort_by(|a, b| {
        let a_name = a
            .as_ref()
            .ok()
            .and_then(|e| e.file_name().into_string().ok())
            .unwrap_or_default();
        let b_name = b
            .as_ref()
            .ok()
            .and_then(|e| e.file_name().into_string().ok())
            .unwrap_or_default();
        a_name.cmp(&b_name)
    });

    println!("Files and directories under {}:", directory);
    for entry in entries {
        match entry {
            Ok(dir_entry) => {
                if let Some(name) = dir_entry.file_name().to_str() {
                    println!("{}", name);
                }
            }
            Err(e) => {
                println!("Error reading entry: {}", e);
            }
        }
    }
}

fn read_file(path: impl AsRef<Path>) {
    let contents = read_to_string(&path)
        .map_err(|e| FaberError::Generic {
            message: format!("Failed to read {}: {}", path.as_ref().display(), e),
        })
        .expect(&format!("Failed to read {}", path.as_ref().display()));

    println!("Read file: {}", contents);
}

pub fn create_faber_cgroup_hierarchy() -> Result<()> {
    let controllers = "+cpu +memory +pids";
    let cgroup_path = PathBuf::from("/sys/fs/cgroup");

    let root_subtree_control_path = cgroup_path.join("cgroup.subtree_control");
    write(root_subtree_control_path, controllers).map_err(|e| FaberError::CgroupControllers {
        e,
        details: "Failed to enable cgroup controllers".to_string(),
    })?;

    let faber_cgroup_path = cgroup_path.join("faber");
    create_dir_all(&faber_cgroup_path).map_err(|e| FaberError::CreateDir {
        e,
        details: "Failed to create faber cgroup directory".to_string(),
    })?;

    let faber_subtree_control = faber_cgroup_path.join("cgroup.subtree_control");
    write(&faber_subtree_control, "+cpu +memory +pids")
        .or_else(|e| {
            if e.raw_os_error() == Some(16) {
                Ok(())
            } else {
                Err(e)
            }
        })
        .map_err(|e| FaberError::CgroupControllerEnable {
            e,
            details: "Failed to enable controllers in faber cgroup".to_string(),
        })?;

    Ok(())
}

pub fn create_task_cgroup(config: &CgroupConfig) -> Result<PathBuf> {
    let faber_cgroup_path = PathBuf::from("/sys/fs/cgroup/faber");

    let task_id = generate_random_string(16);
    let task_cgroup_path = faber_cgroup_path.join(format!("task-{task_id}"));

    // println!("Creating task cgroup: {}", task_cgroup_path);
    create_dir_all(&task_cgroup_path).map_err(|e| FaberError::CreateDir {
        e,
        details: "Failed to create task cgroup directory".to_string(),
    })?;

    let cpu_max_path = task_cgroup_path.join("cpu.max");
    let cpu_max_value = config.cpu_max.as_deref().unwrap_or("50000 100000");

    write(&cpu_max_path, cpu_max_value).map_err(|e| FaberError::WriteFile {
        e,
        details: format!(
            "Failed to write CPU limits '{}' to task cgroup at {}",
            cpu_max_value,
            cpu_max_path.display()
        ),
    })?;

    let memory_max_path = task_cgroup_path.join("memory.max");
    let memory_max_value = config.memory_max.as_deref().unwrap_or("134217728");

    let memory_max_bytes = parse_memory_string(memory_max_value)?;

    write(&memory_max_path, memory_max_bytes.to_string()).map_err(|e| FaberError::WriteFile {
        e,
        details: format!(
            "Failed to write memory limit '{}' bytes to task cgroup",
            memory_max_bytes
        ),
    })?;

    let pids_max_path = task_cgroup_path.join("pids.max");
    let pids_max_value = config
        .pids_max
        .map(|p| p.to_string())
        .unwrap_or_else(|| "64".to_string());
    write(&pids_max_path, &pids_max_value).map_err(|e| FaberError::WriteFile {
        e,
        details: format!(
            "Failed to write PIDs limit '{}' to task cgroup",
            pids_max_value
        ),
    })?;
    Ok(task_cgroup_path)
}

pub fn cleanup_task_cgroup(task_cgroup_path: &str) -> Result<()> {
    remove_dir(task_cgroup_path).map_err(|e| FaberError::RemoveDir {
        e,
        details: "Failed to remove task cgroup directory".to_string(),
    })?;
    Ok(())
}

pub fn add_process_to_task_cgroup(task_cgroup_path: &str, pid: u32) -> Result<()> {
    let cgroup_procs_path = format!("{task_cgroup_path}/cgroup.procs");
    let pid_str = pid.to_string();

    write(&cgroup_procs_path, &pid_str).map_err(|e| FaberError::WriteFile {
        e,
        details: format!("Failed to add process {pid} to task cgroup"),
    })?;

    Ok(())
}

pub fn read_task_stats(task_cgroup_path: &str) -> Result<TaskStats> {
    let mut cpu_stats = CpuStats::default();
    let mut memory_stats = MemoryStats::default();
    let mut pids_stats = PidsStats::default();

    let cpu_stat_path = format!("{task_cgroup_path}/cpu.stat");

    if let Ok(contents) = read_to_string(&cpu_stat_path) {
        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2
                && let Ok(value) = parts[1].parse::<u64>()
            {
                match parts[0] {
                    "usage_usec" => cpu_stats.usage_usec = value,
                    "user_usec" => cpu_stats.user_usec = value,
                    "system_usec" => cpu_stats.system_usec = value,
                    _ => {}
                }
            }
        }
    }

    let memory_current_path = format!("{task_cgroup_path}/memory.current");
    if let Ok(content) = read_to_string(&memory_current_path)
        && let Ok(value) = content.trim().parse::<u64>()
    {
        memory_stats.current = value;
    }

    let memory_peak_path = format!("{task_cgroup_path}/memory.peak");
    if let Ok(content) = read_to_string(&memory_peak_path)
        && let Ok(value) = content.trim().parse::<u64>()
    {
        memory_stats.peak = value;
    }

    let memory_max_path = format!("{task_cgroup_path}/memory.max");
    if let Ok(content) = read_to_string(&memory_max_path)
        && let Ok(value) = content.trim().parse::<u64>()
    {
        memory_stats.max = value;
    }

    let pids_current_path = format!("{task_cgroup_path}/pids.current");
    if let Ok(content) = read_to_string(&pids_current_path)
        && let Ok(value) = content.trim().parse::<u64>()
    {
        pids_stats.current = value;
    }

    let pids_max_path = format!("{task_cgroup_path}/pids.max");
    if let Ok(content) = read_to_string(&pids_max_path)
        && let Ok(value) = content.trim().parse::<u64>()
    {
        pids_stats.max = value;
    }
    Ok(TaskStats {
        cpu: cpu_stats,
        memory: memory_stats,
        pids: pids_stats,
    })
}
