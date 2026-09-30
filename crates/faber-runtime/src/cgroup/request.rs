use std::fs::{create_dir, read_dir, read_to_string, remove_dir, write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::prelude::*;
use crate::utils::generate_random_string;

pub(crate) const REQUEST_CGROUP_PREFIX: &str = "req-";

/// Limits for one request's cgroup subtree. `None` leaves the value at `max`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct RequestLimits {
    pub(crate) memory_max: Option<u64>,
    pub(crate) pids_max: Option<u64>,
}

/// Parent cgroup for every task of one runtime execution.
///
/// Task cgroups are created beneath it, so killing it with the recursive
/// `cgroup.kill` terminates exactly this request and nothing else.
pub(crate) struct RequestCgroup {
    path: PathBuf,
    cleaned: bool,
}

impl RequestCgroup {
    pub(crate) fn new(faber_cgroup_path: &Path, limits: RequestLimits) -> Result<Self> {
        let path = faber_cgroup_path.join(format!(
            "{REQUEST_CGROUP_PREFIX}{}-{}",
            std::process::id(),
            generate_random_string(16)
        ));
        create_dir(&path).map_err(|e| FaberError::CreateDir {
            e,
            details: format!("Failed to create request cgroup at {}", path.display()),
        })?;

        let request_cgroup = Self {
            path,
            cleaned: false,
        };
        write(
            request_cgroup.path.join("cgroup.subtree_control"),
            "+cpu +memory +pids",
        )
        .map_err(|e| FaberError::CgroupControllers {
            e,
            details: "Failed to enable controllers in the request cgroup".to_string(),
        })?;
        for (file, value) in [
            ("memory.max", limits.memory_max),
            ("pids.max", limits.pids_max),
        ] {
            if let Some(value) = value {
                write(request_cgroup.path.join(file), value.to_string()).map_err(|e| {
                    FaberError::WriteFile {
                        e,
                        details: format!("Failed to set {file} on the request cgroup"),
                    }
                })?;
            }
        }

        Ok(request_cgroup)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Kill every process in this request's cgroup subtree.
    pub(crate) fn kill(&self) {
        kill_cgroup_tree(&self.path);
    }

    pub(crate) fn cleanup(mut self) -> Result<()> {
        self.cleaned = true;
        remove_cgroup_tree(&self.path)
    }
}

impl Drop for RequestCgroup {
    fn drop(&mut self) {
        if !self.cleaned {
            self.cleaned = true;
            let _ = remove_cgroup_tree(&self.path);
        }
    }
}

pub(crate) fn is_populated(path: &Path) -> bool {
    read_to_string(path.join("cgroup.events"))
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                let mut fields = line.split_whitespace();
                (fields.next() == Some("populated")).then(|| fields.next() == Some("1"))
            })
        })
        .unwrap_or(false)
}

/// Kill a cgroup and all of its descendants, then wait briefly for the kernel
/// to report the subtree as empty. `cgroup.kill` is recursive.
pub(crate) fn kill_cgroup_tree(path: &Path) {
    if write(path.join("cgroup.kill"), "1").is_err() {
        kill_listed_processes(path);
    }
    let deadline = Instant::now() + Duration::from_millis(500);
    while is_populated(path) && Instant::now() < deadline {
        kill_listed_processes(path);
        thread::sleep(Duration::from_millis(10));
    }
}

fn kill_listed_processes(path: &Path) {
    if let Ok(procs) = read_to_string(path.join("cgroup.procs")) {
        for pid in procs
            .lines()
            .filter_map(|line| line.trim().parse::<i32>().ok())
        {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
    for child in child_cgroups(path) {
        kill_listed_processes(&child);
    }
}

fn child_cgroups(path: &Path) -> Vec<PathBuf> {
    read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .collect()
}

/// Kill a cgroup subtree and remove every directory in it, deepest first.
pub(crate) fn remove_cgroup_tree(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    kill_cgroup_tree(path);

    let mut last_error = None;
    for attempt in 0..10 {
        for child in child_cgroups(path) {
            let _ = remove_cgroup_tree(&child);
        }
        match remove_dir(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        kill_cgroup_tree(path);
        thread::sleep(Duration::from_millis(10 * (attempt + 1)));
    }

    Err(FaberError::RemoveDir {
        e: last_error.unwrap_or_else(|| std::io::Error::other("unknown error")),
        details: format!("Failed to remove cgroup subtree at {}", path.display()),
    })
}
