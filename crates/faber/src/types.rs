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
    /// Executable to run (e.g., `"/bin/sh"` or `"gcc"`).
    pub cmd: String,
    /// Optional list of arguments passed to the command.
    pub args: Option<Vec<String>>,
    /// Environment variables for the command. When not provided, a minimal
    /// environment is used; PATH will be injected if missing.
    pub env: Option<HashMap<String, String>>,
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
    /// Wall execution time in milliseconds.
    pub execution_time_ms: Option<u64>,
    /// Total CPU usage of the task cgroup in microseconds.
    pub cpu_usage_usec: Option<u64>,
    /// User CPU time in microseconds.
    pub cpu_user_usec: Option<u64>,
    /// System CPU time in microseconds.
    pub cpu_system_usec: Option<u64>,
    /// Current memory usage in bytes (from cgroup).
    pub memory_current_bytes: Option<u64>,
    /// Peak memory usage in bytes (from cgroup).
    pub memory_peak_bytes: Option<u64>,
    /// Memory limit in bytes (from cgroup).
    pub memory_limit_bytes: Option<u64>,
}

impl From<Output> for TaskResult {
    fn from(output: Output) -> Self {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Self {
            stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
            execution_time_ms: None,
            cpu_usage_usec: None,
            cpu_user_usec: None,
            cpu_system_usec: None,
            memory_current_bytes: None,
            memory_peak_bytes: None,
            memory_limit_bytes: None,
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

/// Container cgroup configuration.
#[derive(Debug, Clone, Default)]
pub struct CgroupConfig {
    /// Max number of processes allowed.
    pub pids_max: Option<u64>,
    /// Max memory, as a string (e.g., `"256M"`).
    pub memory_max: Option<String>,
    /// CPU max configuration, as a string (e.g., `"max"` or `"20000 100000"`).
    pub cpu_max: Option<String>,
}

/// Runtime-level limits and controls.
#[derive(Debug, Clone, Default)]
pub struct RuntimeLimits {
    /// Time to wait before force-killing stuck processes (wall-clock timeout).
    pub kill_timeout_seconds: Option<u64>,
    /// CPU time limit in milliseconds (CPU time TLE threshold).
    pub cpu_time_limit_ms: Option<u64>,
}
