use crate::prelude::*;
use crate::types::CgroupConfig;
use std::{
    fs::{OpenOptions, create_dir_all, remove_dir},
    io::Write,
    path::{Path, PathBuf},
};

/// Enable one or more controllers at the specified cgroup directory.
pub(crate) fn enable_subtree_controllers_at(dir: &Path, controllers: &[&str]) -> Result<()> {
    let control_path = dir.join("cgroup.subtree_control");
    let controllers = controllers
        .iter()
        .map(|c| format!("+{c}"))
        .collect::<Vec<String>>()
        .join(" ");

    write_file(&control_path, controllers.as_bytes())?;
    Ok(())
}

/// Create a leaf cgroup under the given base directory.
pub(crate) fn create_cgroup_at(base: &PathBuf) -> Result<()> {
    create_dir_all(base).map_err(|source| Error::CreateDir {
        path: base.clone(),
        source,
        details: "Failed to create cgroup".to_string(),
    })?;

    Ok(())
}

/// Apply limits for pids, memory, and cpu to a cgroup leaf according to the config.
pub(crate) fn set_limits(cg: &Path, cfg: &CgroupConfig) -> Result<()> {
    if let Some(pids) = cfg.pids_max {
        let pids_path = cg.join("pids.max");
        write_file(&pids_path, pids.to_string().as_bytes())?;
    }

    if let Some(mem) = &cfg.memory_max {
        let mem_path = cg.join("memory.max");
        write_file(&mem_path, mem.to_string().as_bytes())?;
    }
    if let Some(cpu) = &cfg.cpu_max {
        let cpu_path = cg.join("cpu.max");
        write_file(&cpu_path, cpu.to_string().as_bytes())?;
    }
    Ok(())
}

pub(crate) fn remove_cgroup(cg: &Path) -> Result<()> {
    remove_dir(cg).map_err(|source| Error::RemoveDir {
        path: cg.to_path_buf(),
        source,
        details: "Failed to remove cgroup".to_string(),
    })?;
    Ok(())
}

/// Add a process to the cgroup by pid, verifying membership and falling back to cgroup.threads when needed.
pub(crate) fn add_pid(cg: &Path, pid: i32) -> Result<()> {
    let procs_path = cg.join("cgroup.procs");
    let threads_path = cg.join("cgroup.threads");

    // Path that should appear in /proc/<pid>/cgroup (cgroup v2 format: 0::/path)
    let expected_rel = match cg.strip_prefix("/sys/fs/cgroup") {
        Ok(p) => PathBuf::from("/").join(p),
        Err(_) => cg.to_path_buf(),
    };
    let expected_str = expected_rel.to_string_lossy().to_string();

    // Try writing to cgroup.procs first
    if let Err(e) = write_file(&procs_path, pid.to_string().as_bytes()) {
        // If not supported, attempt cgroup.threads
        if e.to_string().contains("95") || e.to_string().contains("EOPNOTSUPP") {
            if let Err(e2) = write_file(&threads_path, pid.to_string().as_bytes()) {
                return Err(Error::Io {
                    operation: "write cgroup.threads".to_string(),
                    path: threads_path.to_string_lossy().to_string(),
                    details: format!("{e2}"),
                });
            }
        } else {
            return Err(Error::Io {
                operation: "write cgroup.procs".to_string(),
                path: procs_path.to_string_lossy().to_string(),
                details: format!("{e}"),
            });
        }
    }

    // Verify process membership
    let proc_cgroup_path = format!("/proc/{}/cgroup", pid);
    if let Ok(contents) = std::fs::read_to_string(&proc_cgroup_path) {
        if contents.contains(&expected_str) {
            return Ok(());
        }
    }

    // Fallback: try writing to cgroup.threads explicitly if not yet attempted
    if let Err(e3) = write_file(&threads_path, pid.to_string().as_bytes()) {
        return Err(Error::Io {
            operation: "write cgroup.threads".to_string(),
            path: threads_path.to_string_lossy().to_string(),
            details: format!("{e3}"),
        });
    }

    // Re-verify
    if let Ok(contents) = std::fs::read_to_string(&proc_cgroup_path) {
        if contents.contains(&expected_str) {
            return Ok(());
        }
    }

    Err(Error::Io {
        operation: "verify cgroup membership".to_string(),
        path: expected_str,
        details: format!("pid {pid} not found in cgroup after attach"),
    })
}

// Small helper to write to files in cgroup directories.
fn write_file(path: &Path, contents: &[u8]) -> Result<()> {
    let mut f = OpenOptions::new()
        .create(false)
        .write(true)
        .open(&path)
        .map_err(|e| Error::Io {
            operation: format!("open {path:?}"),
            path: path.to_string_lossy().to_string(),
            details: format!("{e}"),
        })?;
    f.write_all(contents).map_err(|e| Error::Io {
        operation: format!("write {path:?}"),
        path: path.to_string_lossy().to_string(),
        details: format!("{e}"),
    })?;
    f.flush().map_err(|e| Error::Io {
        operation: format!("flush {path:?}"),
        path: path.to_string_lossy().to_string(),
        details: format!("{e}"),
    })?;
    Ok(())
}

// TODO: Remove below functions
// Debug helper - will be removed
pub(crate) fn list_files(path: &Path) {
    let entries = std::fs::read_dir(path).unwrap();
    for entry in entries {
        let entry = entry.unwrap();
        eprintln!("[faber][debug] list_files: {}", entry.path().display());
    }
}

pub(crate) fn debug(path: &Path) {
    let contents = std::fs::read_to_string(path).unwrap_or_default();
    eprintln!("[faber][debug]debug: {path:?} = {contents}");
}
