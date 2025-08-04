use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub command: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
}

/// Resource usage statistics for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU time used in nanoseconds
    pub cpu_time_ns: u64,
    /// Wall clock time in nanoseconds
    pub wall_time_ns: u64,
    /// Peak memory usage in bytes
    pub memory_peak_bytes: u64,
    /// Current memory usage in bytes
    pub memory_current_bytes: u64,
    /// Number of processes created
    pub process_count: u32,
    /// Number of file descriptors used
    pub file_descriptors: u32,
    /// I/O read bytes
    pub io_read_bytes: u64,
    /// I/O write bytes
    pub io_write_bytes: u64,
    /// System time in nanoseconds
    pub system_time_ns: u64,
    /// User time in nanoseconds
    pub user_time_ns: u64,
}

impl ResourceUsage {
    pub fn new() -> Self {
        Self {
            cpu_time_ns: 0,
            wall_time_ns: 0,
            memory_peak_bytes: 0,
            memory_current_bytes: 0,
            process_count: 0,
            file_descriptors: 0,
            io_read_bytes: 0,
            io_write_bytes: 0,
            system_time_ns: 0,
            user_time_ns: 0,
        }
    }

    /// Get CPU time as Duration
    pub fn cpu_time(&self) -> Duration {
        Duration::from_nanos(self.cpu_time_ns)
    }

    /// Get wall time as Duration
    pub fn wall_time(&self) -> Duration {
        Duration::from_nanos(self.wall_time_ns)
    }

    /// Get memory usage in MB
    pub fn memory_peak_mb(&self) -> f64 {
        self.memory_peak_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get current memory usage in MB
    pub fn memory_current_mb(&self) -> f64 {
        self.memory_current_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get I/O read in MB
    pub fn io_read_mb(&self) -> f64 {
        self.io_read_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get I/O write in MB
    pub fn io_write_mb(&self) -> f64 {
        self.io_write_bytes as f64 / (1024.0 * 1024.0)
    }
}

#[derive(Debug, Serialize)]
pub struct TaskResult {
    pub status: TaskStatus,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    /// Resource usage statistics
    pub resource_usage: ResourceUsage,
    /// Whether the task exceeded any resource limits
    pub resource_limits_exceeded: ResourceLimitViolations,
}

/// Resource limit violations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitViolations {
    pub cpu_time_limit_exceeded: bool,
    pub wall_time_limit_exceeded: bool,
    pub process_limit_exceeded: bool,
    pub file_descriptor_limit_exceeded: bool,
    pub output_limit_exceeded: bool,
}

impl ResourceLimitViolations {
    pub fn new() -> Self {
        Self {
            cpu_time_limit_exceeded: false,
            wall_time_limit_exceeded: false,
            process_limit_exceeded: false,
            file_descriptor_limit_exceeded: false,
            output_limit_exceeded: false,
        }
    }

    pub fn any_exceeded(&self) -> bool {
        self.cpu_time_limit_exceeded
            || self.wall_time_limit_exceeded
            || self.process_limit_exceeded
            || self.file_descriptor_limit_exceeded
            || self.output_limit_exceeded
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Success,
    Failure,
    NotExecuted,
    /// Task failed due to resource limits
    ResourceLimitExceeded,
    /// Task was killed due to timeout
    Timeout,
    /// Task was killed due to memory limit
    MemoryLimitExceeded,
    /// Task was killed due to CPU limit
    CpuLimitExceeded,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskStatus::Success => write!(f, "Success"),
            TaskStatus::Failure => write!(f, "Failure"),
            TaskStatus::NotExecuted => write!(f, "NotExecuted"),
            TaskStatus::ResourceLimitExceeded => write!(f, "ResourceLimitExceeded"),
            TaskStatus::Timeout => write!(f, "Timeout"),
            TaskStatus::MemoryLimitExceeded => write!(f, "MemoryLimitExceeded"),
            TaskStatus::CpuLimitExceeded => write!(f, "CpuLimitExceeded"),
        }
    }
}

// Type aliases for backward compatibility
pub type ExecutionTask = Task;
pub type ExecutionTaskResult = TaskResult;
pub type ExecutionTaskStatus = TaskStatus;
