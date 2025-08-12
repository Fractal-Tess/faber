use std::{collections::HashMap, process::Output};

use nix::mount::MsFlags;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cwd: Option<String>,
    pub stdin: Option<String>,
    pub files: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl From<Output> for TaskResult {
    fn from(output: Output) -> Self {
        Self {
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
            exit_code: output.status.code().unwrap_or(-1),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mount {
    pub source: String,
    pub target: String,
    pub flags: Vec<MsFlags>,
    pub options: Vec<MsFlags>,
    pub data: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FilesystemConfig {
    pub tmp_size: String,
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

#[derive(Debug, Clone, Default)]
pub struct CgroupConfig {
    pub enabled: bool,
    pub pids_max: Option<u64>,
    pub memory_max: Option<String>,
    pub cpu_max: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeLimits {
    pub kill_timeout_seconds: Option<u64>,
}
