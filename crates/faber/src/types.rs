use std::{collections::HashMap, process::Output};

use nix::mount::MsFlags;
use serde::{Deserialize, Serialize};

/// A single command to execute inside the sandboxed runtime.
///
/// Supply the command binary via `cmd` and optionally arguments, environment,
/// working directory, stdin and a set of files to place into the workdir prior
/// to execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Executable to run (e.g. `"/bin/sh"` or `"gcc"`).
    pub cmd: String,
    /// Optional list of arguments passed to the command.
    pub args: Option<Vec<String>>,
    /// Environment variables for the command. When not provided, a minimal
    /// environment is used; PATH will be injected if missing.
    pub env: Option<HashMap<String, String>>,
    /// Current working directory for the command (inside the container view).
    pub cwd: Option<String>,
    /// Optional stdin contents to feed to the command.
    pub stdin: Option<String>,
    /// Files to materialize relative to the workdir before running the task.
    pub files: Option<HashMap<String, String>>,
}

/// Output of a finished task, including captured stdout, stderr and exit code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Captured standard output as UTF-8 string.
    pub stdout: String,
    /// Captured standard error as UTF-8 string.
    pub stderr: String,
    /// Process exit code. `-1` if not available.
    pub exit_code: i32,
}

impl From<Output> for TaskResult {
    fn from(output: Output) -> Self {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Self {
            stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
        }
    }
}

/// Bind mount specification to expose host paths inside the container view.
#[derive(Debug, Clone)]
pub struct Mount {
    /// Source path on the host.
    pub source: String,
    /// Target path inside the container namespace.
    pub target: String,
    /// Mount flags applied to this bind mount.
    pub flags: Vec<MsFlags>,
    /// Additional mount options flags (kept for extensibility).
    pub options: Vec<MsFlags>,
    /// Optional fs-specific data string.
    pub data: Option<String>,
}

/// Size limits for tmpfs mounts used by the runtime.
#[derive(Debug, Clone)]
pub struct FilesystemConfig {
    /// Size for `/tmp` tmpfs (e.g., `"128M"`, `"1G"`).
    pub tmp_size: String,
    /// Size for workdir tmpfs (e.g., `"256M"`).
    pub workdir_size: String,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            tmp_size: "128M".to_string(),
            workdir_size: "256M".to_string(),
        }
    }
}

/// Container cgroup configuration (placeholder; not yet wired fully).
#[derive(Debug, Clone, Default)]
pub struct CgroupConfig {
    /// Enable or disable cgroup limits.
    pub enabled: bool,
    /// Max number of processes allowed.
    pub pids_max: Option<u64>,
    /// Max memory, as a string (e.g., `"256M"`).
    pub memory_max: Option<String>,
    /// CPU max configuration, as a string (e.g., `"max 100000"`).
    pub cpu_max: Option<String>,
}

/// Runtime-level limits and controls.
#[derive(Debug, Clone, Default)]
pub struct RuntimeLimits {
    /// Time to wait before force-killing stuck processes.
    pub kill_timeout_seconds: Option<u64>,
}
