use std::collections::HashMap;

use nix::mount::MsFlags;

#[derive(Debug, Clone)]
pub struct Task {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cwd: Option<String>,
    pub stdin: Option<String>,
    pub files: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub struct Mount {
    pub source: String,
    pub target: String,
    pub flags: Vec<MsFlags>,
    pub options: Vec<MsFlags>,
    pub data: Option<String>,
}
