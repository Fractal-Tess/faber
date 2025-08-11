use crate::prelude::*;
use std::{
    fs::{OpenOptions, create_dir_all},
    io::Write,
    path::PathBuf,
};

#[derive(Debug)]
pub(crate) struct CgroupManager {
    pub(crate) cgroup_path: PathBuf,
    pub(crate) pids_max: u64,
}

impl CgroupManager {
    pub fn initilize(&self) -> Result<()> {
        // Create the cgroup directory on the host
        println!(
            "[DEBUG] Creating cgroup directory at: {}",
            self.cgroup_path.display()
        );

        create_dir_all(&self.cgroup_path).map_err(|e| {
            Error::Cgroup(format!(
                "Failed to create container cgroup at '{}': {e}",
                self.cgroup_path.display()
            ))
        })?;

        println!("[DEBUG] Cgroup directory created successfully");
        println!("[DEBUG] Directory exists: {}", self.cgroup_path.exists());
        println!(
            "[DEBUG] Directory contents: {:?}",
            std::fs::read_dir(&self.cgroup_path)
                .map(|d| d.collect::<Vec<_>>().len())
                .unwrap_or(0)
        );

        // Set the configured PID limit
        println!("[DEBUG] Setting PID limit to: {}", self.pids_max);
        self.set_max_procs(self.pids_max)?;
        println!("[DEBUG] PID limit set successfully");

        Ok(())
    }

    pub fn add_host_process(&self, pid: i32) -> Result<()> {
        // Add a process to the cgroup from the host side
        // Skip the process existence check since we're adding from host context
        let cgroup_procs = self.cgroup_path.join("cgroup.procs");

        let mut file = OpenOptions::new()
            .append(true)
            .truncate(false)
            .open(&cgroup_procs)
            .map_err(|e| {
                Error::Cgroup(format!(
                    "Failed to open cgroup.procs at '{}': {e}",
                    cgroup_procs.display()
                ))
            })?;

        // Write the PID as bytes with newline
        file.write_all(format!("{pid}\n").as_bytes())
            .map_err(|e| Error::Cgroup(format!("Cannot write PID {}: {}", pid, e)))?;

        // Ensure immediate flush
        file.sync_all()
            .map_err(|e| Error::Cgroup(format!("Cannot sync cgroup.procs: {}", e)))?;

        Ok(())
    }

    pub fn add_proc(&self, pid: i32) -> Result<()> {
        // Check if process exists first
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            return Err(Error::ProcessManagement {
                operation: "add process to cgroup".to_string(),
                pid,
                details: "Process does not exist in /proc".to_string(),
            });
        }

        let cgroup_procs = self.cgroup_path.join("cgroup.procs");

        let mut file = OpenOptions::new()
            .append(true)
            .truncate(false)
            .open(&cgroup_procs)
            .map_err(|e| {
                Error::Cgroup(format!(
                    "Failed to open cgroup.procs at '{}': {e}",
                    cgroup_procs.display()
                ))
            })?;

        // Write the PID as bytes with newline
        file.write_all(format!("{pid}\n").as_bytes()).map_err(|e| {
            Error::Cgroup(format!(
                "Failed to write PID {pid} to cgroup.procs at '{}': {e}",
                cgroup_procs.display()
            ))
        })?;

        // Ensure immediate flush
        file.sync_all().map_err(|e| {
            Error::Cgroup(format!(
                "Failed to sync cgroup.procs at '{}' after adding PID {pid}: {e}",
                cgroup_procs.display()
            ))
        })?;

        Ok(())
    }

    pub fn set_max_procs(&self, limit: u64) -> Result<()> {
        let cgroup_pids = self.cgroup_path.join("pids.max");

        // Check if the pids.max file exists
        if !cgroup_pids.exists() {
            return Err(Error::Cgroup(format!(
                "pids.max file not found in cgroup. Path: {:?}",
                self.cgroup_path
            )));
        }

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(false)
            .open(&cgroup_pids)
            .map_err(|e| {
                Error::Cgroup(format!(
                    "Failed to open pids.max at '{}' for writing limit {limit}: {e}",
                    cgroup_pids.display()
                ))
            })?;

        file.write_all(format!("{limit}\n").as_bytes())
            .map_err(|e| {
                Error::Cgroup(format!(
                    "Failed to write limit {limit} to pids.max at '{}': {e}",
                    cgroup_pids.display()
                ))
            })?;

        file.sync_all().map_err(|e| {
            Error::Cgroup(format!(
                "Failed to sync pids.max at '{}' after setting limit {limit}: {e}",
                cgroup_pids.display()
            ))
        })?;

        Ok(())
    }

    pub fn cleanup(&self) -> Result<()> {
        // Check if the cgroup directory exists before trying to remove it
        if self.cgroup_path.exists() {
            match std::fs::remove_dir_all(&self.cgroup_path) {
                Ok(_) => {}
                Err(e) => {
                    println!("Failed to remove cgroup directory: {e}");
                }
            }
        };
        Ok(())
    }

    pub fn get_current_pids(&self) -> Result<u64> {
        let cgroup_pids = self.cgroup_path.join("pids.current");

        if !cgroup_pids.exists() {
            return Err(Error::Cgroup(format!(
                "pids.current file not found in cgroup. Path: {:?}",
                self.cgroup_path
            )));
        }

        let content = std::fs::read_to_string(&cgroup_pids).map_err(|e| {
            Error::Cgroup(format!(
                "Failed to read pids.current from '{}': {e}",
                cgroup_pids.display()
            ))
        })?;

        content.trim().parse::<u64>().map_err(|e| {
            Error::Cgroup(format!(
                "Failed to parse pids.current value '{}': {e}",
                content.trim()
            ))
        })
    }

    pub fn check_pid_limit(&self) -> Result<bool> {
        let current = self.get_current_pids()?;
        Ok(current < self.pids_max)
    }

    pub fn enforce_pid_limit(&self) -> Result<()> {
        if !self.check_pid_limit()? {
            return Err(Error::Cgroup(format!(
                "PID limit exceeded: current {} >= max {}",
                self.get_current_pids()?,
                self.pids_max
            )));
        }
        Ok(())
    }

    pub fn add_child_process(&self, pid: i32) -> Result<()> {
        // Add child process to cgroup and check limits
        self.add_proc(pid)?;

        // Enforce PID limit after adding
        self.enforce_pid_limit()?;

        Ok(())
    }

    pub fn get_pid_stats(&self) -> Result<(u64, u64)> {
        let current = self.get_current_pids()?;
        Ok((current, self.pids_max))
    }
}
