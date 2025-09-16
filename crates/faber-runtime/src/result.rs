use serde::{Deserialize, Serialize};

pub type TaskGroupResult = Vec<ExecutionStepResult>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ExecutionStepResult {
    Single(TaskResult),
    Parallel(Vec<TaskResult>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum TaskResult {
    Completed {
        stdout: String,
        stderr: String,
        exit_code: i32,
        stats: TaskResultStats,
    },
    Failed {
        error: String,
        stats: TaskResultStats,
    },
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TaskResultStats {
    pub memory_peak_bytes: u64,
    pub cpu_usage_percent: f64,
    pub execution_time_ms: u64,
}
