use std::collections::HashMap;

use nix::mount::MsFlags;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct Mount {
    pub source: String,
    pub target: String,
    pub flags: Vec<MsFlags>,
    pub options: Vec<MsFlags>,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CgroupConfig {
    pub enabled: bool,
    pub pids_max: Option<String>,
    pub memory_max: Option<String>,
    pub cpu_max: Option<String>,
}
