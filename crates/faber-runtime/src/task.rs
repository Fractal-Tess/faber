use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type TaskGroup = Vec<ExecutionStep>;

#[derive(Debug, Clone)]
pub enum ExecutionStep {
    Single(Task),
    Parallel(Vec<Task>),
}

impl serde::Serialize for ExecutionStep {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ExecutionStep::Single(task) => task.serialize(serializer),
            ExecutionStep::Parallel(tasks) => tasks.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ExecutionStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let value = serde_json::Value::deserialize(deserializer)?;

        match value {
            serde_json::Value::Object(_) => {
                let task = Task::deserialize(value).map_err(Error::custom)?;
                Ok(ExecutionStep::Single(task))
            }
            serde_json::Value::Array(_) => {
                let tasks = Vec::<Task>::deserialize(value).map_err(Error::custom)?;
                Ok(ExecutionStep::Parallel(tasks))
            }
            _ => Err(Error::custom(
                "ExecutionStep must be either an object (Single) or an array (Parallel)",
            )),
        }
    }
}

/// Versioned seccomp policy for a task.
///
/// `v1` profiles are denylists: known dangerous syscalls kill the task and
/// everything else is allowed. `v2` profiles add an allowlist on top: a
/// syscall that is neither listed nor denied fails with `ENOSYS`, as it
/// would on an older kernel. `compile` profiles may create processes and
/// namespace-scoped sockets; `native` profiles may do neither.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxProfile {
    CompileV1,
    NativeV1,
    #[default]
    CompileV2,
    NativeV2,
}

impl SandboxProfile {
    /// Whether tasks may fork, exec helpers and open namespace-scoped sockets.
    pub fn allows_processes(self) -> bool {
        matches!(self, Self::CompileV1 | Self::CompileV2)
    }

    /// Whether unlisted syscalls are refused rather than allowed.
    pub fn has_allowlist(self) -> bool {
        matches!(self, Self::CompileV2 | Self::NativeV2)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub stdin: Option<String>,
    pub files: Option<HashMap<String, String>>,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub sandbox_profile: Option<SandboxProfile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskStats {
    pub cpu_usage_usec: u64,
    pub cpu_nr_throttled: u64,
    pub cpu_throttled_usec: u64,
    pub memory_peak_bytes: u64,
    pub pids_max: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TaskCgroupEvents {
    /// Times this task cgroup's own `memory.max` was reached and could not be
    /// reclaimed. Zero with a nonzero `oom_kill_count` means an ancestor's
    /// limit (request, service, container, or host) chose the victim.
    pub own_oom_count: u64,
    pub oom_kill_count: u64,
    pub pids_limit_hit_count: u64,
}
