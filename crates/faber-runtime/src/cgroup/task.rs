use std::fs::{create_dir_all, read_to_string, remove_dir, write};
use std::path::PathBuf;

use super::config::CgroupConfig;
use crate::prelude::*;
use crate::task::TaskStats;
use crate::utils::generate_random_string;

pub struct TaskCgroup {
    task_cgroup_path: PathBuf,
    config: CgroupConfig,
}

impl TaskCgroup {
    pub fn new(config: CgroupConfig) -> Result<Self> {
        let faber_cgroup_path = PathBuf::from("/sys/fs/cgroup/faber");
        let task_id = generate_random_string(16);
        let task_cgroup_path = faber_cgroup_path.join(format!("task-{task_id}"));

        create_dir_all(&task_cgroup_path).map_err(|e| FaberError::CreateDir {
            e,
            details: "Failed to create task cgroup directory".to_string(),
        })?;

        let task_cgroup = Self {
            task_cgroup_path,
            config,
        };

        task_cgroup.setup_cgroup_files()?;

        Ok(task_cgroup)
    }

    pub fn add_process(&self, pid: u32) -> Result<()> {
        let cgroup_procs_path = self.task_cgroup_path.join("cgroup.procs");
        let pid_str = pid.to_string();

        write(&cgroup_procs_path, &pid_str).map_err(|e| FaberError::WriteFile {
            e,
            details: format!("Failed to add process {pid} to task cgroup"),
        })?;

        Ok(())
    }

    pub fn measure_resources(&self) -> Result<TaskStats> {
        let mut cpu_usage_usec = 0u64;
        let mut memory_peak_bytes = 0u64;
        let mut pids_max = 0u64;

        let cpu_stat_path = self.task_cgroup_path.join("cpu.stat");
        if let Ok(contents) = read_to_string(&cpu_stat_path) {
            for line in contents.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() == 2
                    && let Ok(value) = parts[1].parse::<u64>()
                    && parts[0] == "usage_usec"
                {
                    cpu_usage_usec = value;
                }
            }
        }

        let memory_peak_path = self.task_cgroup_path.join("memory.peak");
        if let Ok(content) = read_to_string(&memory_peak_path)
            && let Ok(value) = content.trim().parse::<u64>()
        {
            memory_peak_bytes = value;
        }

        let pids_max_path = self.task_cgroup_path.join("pids.peak");
        if let Ok(content) = read_to_string(&pids_max_path)
            && let Ok(value) = content.trim().parse::<u64>()
        {
            pids_max = value;
        }

        Ok(TaskStats {
            cpu_usage_usec,
            memory_peak_bytes,
            pids_max,
        })
    }

    pub fn cleanup(self) -> Result<()> {
        remove_dir(&self.task_cgroup_path).map_err(|e| FaberError::RemoveDir {
            e,
            details: "Failed to remove task cgroup directory".to_string(),
        })?;
        Ok(())
    }

    fn setup_cgroup_files(&self) -> Result<()> {
        let cpu_max_path = self.task_cgroup_path.join("cpu.max");

        write(&cpu_max_path, &self.config.cpu_max).map_err(|e| FaberError::WriteFile {
            e,
            details: format!(
                "Failed to write CPU limits '{}' to task cgroup at {}",
                self.config.cpu_max,
                cpu_max_path.display()
            ),
        })?;

        let memory_max_path = self.task_cgroup_path.join("memory.max");

        let memory_max_bytes = self.parse_memory_string(&self.config.memory_max)?;

        write(&memory_max_path, memory_max_bytes.to_string()).map_err(|e| {
            FaberError::WriteFile {
                e,
                details: format!(
                    "Failed to write memory limit '{}' bytes to task cgroup",
                    memory_max_bytes
                ),
            }
        })?;

        let pids_max_path = self.task_cgroup_path.join("pids.max");
        let pids_max_value = self.config.pids_max.to_string();

        write(&pids_max_path, &pids_max_value).map_err(|e| FaberError::WriteFile {
            e,
            details: format!(
                "Failed to write PIDs limit '{}' to task cgroup",
                pids_max_value
            ),
        })?;

        Ok(())
    }

    fn parse_memory_string(&self, memory_str: &str) -> Result<u64> {
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
}
