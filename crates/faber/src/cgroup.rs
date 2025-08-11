use crate::prelude::*;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Debug)]
pub struct CgroupManager {
    cgroup_path: PathBuf,
}

impl CgroupManager {
    pub fn new(id: impl Into<String>) -> Result<Self> {
        let id_string = id.into();
        println!("=== Creating CgroupManager for ID: {} ===", id_string);

        // Create container cgroup directly under root with pattern 'faber-<id>'
        let cgroup_path = PathBuf::from(format!("/sys/fs/cgroup/faber-{}", id_string));
        println!("Container cgroup path: {:?}", cgroup_path);

        // Create the cgroup directory if it doesn't exist
        println!("Creating container cgroup directory...");
        std::fs::create_dir_all(&cgroup_path)
            .map_err(|e| Error::Generic(format!("Failed to create container cgroup: {e}")))?;

        // Set initial pids.max to a reasonable default
        let pids_max_path = cgroup_path.join("pids.max");
        if pids_max_path.exists() {
            println!("Setting initial pids.max to 100...");
            let mut file = OpenOptions::new()
                .write(true)
                .open(&pids_max_path)
                .map_err(|e| Error::Generic(format!("Failed to open pids.max: {e}")))?;

            file.write_all(b"100\n")
                .map_err(|e| Error::Generic(format!("Failed to write to pids.max: {e}")))?;

            file.flush()
                .map_err(|e| Error::Generic(format!("Failed to sync pids.max: {e}")))?;

            println!("✓ Initial pids.max set to 100");
        } else {
            println!("⚠️  pids.max not found - cgroup may not have pids controller enabled");
        }

        println!("=== CgroupManager creation complete ===");
        Ok(CgroupManager { cgroup_path })
    }

    pub fn add_proc(&self, pid: u32) -> Result<()> {
        // Check if process exists first
        if !PathBuf::from(format!("/proc/{pid}")).exists() {
            return Err(Error::Generic(format!("Process {pid} does not exist")));
        }

        let cgroup_procs = self.cgroup_path.join("cgroup.procs");

        let mut file = OpenOptions::new()
            .append(true)
            .truncate(false)
            .open(&cgroup_procs)?;

        // Write the PID as bytes with newline
        file.write_all(format!("{pid}\n").as_bytes())
            .map_err(|e| Error::Generic(format!("Failed to write to cgroup.procs: {e}")))?;

        // Ensure immediate flush
        file.sync_all()
            .map_err(|e| Error::Generic(format!("Failed to sync cgroup.procs: {e}")))?;

        Ok(())
    }

    pub fn print_group_procs(&self) -> Result<()> {
        let procs_path = self.cgroup_path.join("cgroup.procs");

        let mut file = OpenOptions::new().read(true).open(procs_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        println!("Cgroup procs: {}", contents);

        Ok(())
    }

    pub fn set_max_procs(&self, limit: u64) -> Result<()> {
        let cgroup_pids = self.cgroup_path.join("pids.max");

        // Check if the pids.max file exists
        if !cgroup_pids.exists() {
            return Err(Error::Generic(format!(
                "pids.max file not found in cgroup. Path: {:?}",
                self.cgroup_path
            )));
        }

        let mut file = OpenOptions::new()
            .write(true)
            .truncate(false)
            .open(&cgroup_pids)
            .map_err(|e| Error::Generic(format!("Failed to open pids.max: {e}")))?;

        file.write_all(format!("{limit}\n").as_bytes())
            .map_err(|e| Error::Generic(format!("Failed to write to pids.max: {e}")))?;

        file.sync_all()
            .map_err(|e| Error::Generic(format!("Failed to sync pids.max: {e}")))?;

        Ok(())
    }

    pub fn print_max_procs(&self) -> Result<()> {
        let pids_path = self.cgroup_path.join("pids.max");

        let mut file = OpenOptions::new().read(true).open(pids_path)?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        println!("Cgroup pids: {contents}");

        Ok(())
    }
}
