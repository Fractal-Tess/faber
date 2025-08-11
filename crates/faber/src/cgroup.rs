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
        // Create the cgroup directory
        create_dir_all(&self.cgroup_path).map_err(|e| {
            Error::Cgroup(format!(
                "Failed to create container cgroup at '{}': {e}",
                self.cgroup_path.display()
            ))
        })?;

        // Set initial pids.max to a reasonable default
        let pids_max_path = self.cgroup_path.join("pids.max");

        let mut file = OpenOptions::new()
            .write(true)
            .open(&pids_max_path)
            .map_err(|e| {
                Error::Cgroup(format!(
                    "Failed to open pids.max at '{}': {e}",
                    pids_max_path.display()
                ))
            })?;

        file.write_all(b"100\n").map_err(|e| {
            Error::Cgroup(format!(
                "Failed to write to pids.max at '{}': {e}",
                pids_max_path.display()
            ))
        })?;

        file.flush().map_err(|e| {
            Error::Cgroup(format!(
                "Failed to sync pids.max at '{}': {e}",
                pids_max_path.display()
            ))
        })?;

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
}
