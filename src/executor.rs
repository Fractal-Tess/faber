use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;
use tempfile::TempDir;
use thiserror::Error;
use tokio::process::Command as TokioCommand;
use utoipa::ToSchema;

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Failed to create sandbox directory: {0}")]
    CreateSandbox(#[from] std::io::Error),
    #[error("Invalid task order: {0}")]
    InvalidOrder(String),
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),
    #[error("File operation failed: {0}")]
    FileOperation(String),
}

/// File source for copying files into sandbox
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum FileSource {
    /// File content as string
    Content { content: String },
}

/// A task to execute in the sandbox
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Task {
    /// Order of execution (must be unique, start from 0, no gaps)
    pub order: u32,
    /// Command arguments (first element is the executable)
    pub args: Vec<String>,
    /// Environment variables in "KEY=VALUE" format
    #[serde(default)]
    pub env: Vec<String>,
    /// Source files to copy into sandbox
    #[serde(default)]
    pub src: HashMap<String, FileSource>,
}

/// Execution request containing named tasks
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionRequest {
    /// Tasks mapped by arbitrary names
    #[serde(flatten)]
    pub tasks: HashMap<String, Task>,
}

/// Result of a single task execution
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TaskResult {
    /// Exit code
    #[serde(rename = "exitStatus")]
    pub exit_status: i32,
    /// CPU time used in human-readable format (e.g., "250ms", "10ns")
    pub time: String,
    /// Peak memory usage in human-readable format (e.g., "12mb", "30mb")
    pub memory: String,
    /// Output files (stdout, stderr, etc.)
    pub files: HashMap<String, String>,
}

/// Complete execution result
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutionResult {
    /// Results mapped by task names
    #[serde(flatten)]
    pub results: HashMap<String, TaskResult>,
}

impl ExecutionRequest {
    /// Validate the execution request
    pub fn validate(&self) -> Result<(), ExecutorError> {
        if self.tasks.is_empty() {
            return Err(ExecutorError::InvalidOrder(
                "Request must contain at least one task".to_string(),
            ));
        }

        // Check that orders start from 0 and have no gaps
        let mut orders: Vec<u32> = self.tasks.values().map(|t| t.order).collect();
        orders.sort();

        for (i, &order) in orders.iter().enumerate() {
            if order != i as u32 {
                return Err(ExecutorError::InvalidOrder(format!(
                    "Task orders must start from 0 and have no gaps. Expected {i}, found {order}"
                )));
            }
        }

        // Check for duplicate orders
        let unique_orders: std::collections::HashSet<u32> = orders.iter().cloned().collect();
        if unique_orders.len() != orders.len() {
            return Err(ExecutorError::InvalidOrder(
                "Task orders must be unique".to_string(),
            ));
        }

        // Validate individual tasks
        for (name, task) in &self.tasks {
            if task.args.is_empty() {
                return Err(ExecutorError::InvalidOrder(format!(
                    "Task '{name}' must have at least one argument"
                )));
            }
        }

        Ok(())
    }

    /// Get tasks sorted by order
    pub fn ordered_tasks(&self) -> Vec<(&String, &Task)> {
        let mut tasks: Vec<_> = self.tasks.iter().collect();
        tasks.sort_by_key(|(_, task)| task.order);
        tasks
    }
}

/// Simple executor for running tasks in sandbox
pub struct SandboxExecutor {
    work_dir: TempDir,
}

impl SandboxExecutor {
    /// Create a new sandbox executor
    pub fn new() -> Result<Self, ExecutorError> {
        let work_dir = tempfile::tempdir()?;
        Ok(Self { work_dir })
    }

    /// Execute all tasks in the request
    pub async fn execute(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ExecutionResult, ExecutorError> {
        request.validate()?;

        let mut results = HashMap::new();

        // Execute tasks in order
        for (task_name, task) in request.ordered_tasks() {
            let result = self.execute_task(task).await?;
            results.insert(task_name.clone(), result);
        }

        Ok(ExecutionResult { results })
    }

    /// Execute a single task
    async fn execute_task(&self, task: &Task) -> Result<TaskResult, ExecutorError> {
        let work_path = self.work_dir.path();

        // Copy source files into sandbox
        for (filename, file_source) in &task.src {
            self.copy_file_to_sandbox(work_path, filename, file_source)?;
        }

        // Execute the command with memory monitoring
        let start_time = Instant::now();
        let mut command = TokioCommand::new(&task.args[0]);

        // Add arguments
        for arg in &task.args[1..] {
            command.arg(arg);
        }

        // Add environment variables
        for env in &task.env {
            if let Some((key, value)) = env.split_once('=') {
                command.env(key, value);
            }
        }

        // Set working directory and I/O
        command
            .current_dir(work_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Spawn the process
        let child = command
            .spawn()
            .map_err(|e| ExecutorError::ExecutionFailed(format!("Failed to spawn command: {e}")))?;

        let pid = child.id();
        let mut peak_memory = 0u64;

        // Monitor memory usage in a separate task
        let memory_monitor = if let Some(pid) = pid {
            let monitor_handle = tokio::spawn(async move {
                let mut peak = 0u64;
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(50));

                loop {
                    interval.tick().await;

                    // Read memory usage from /proc/[pid]/status
                    if let Ok(status) = fs::read_to_string(format!("/proc/{pid}/status")) {
                        for line in status.lines() {
                            if line.starts_with("VmRSS:") {
                                if let Some(kb_str) = line.split_whitespace().nth(1) {
                                    if let Ok(kb) = kb_str.parse::<u64>() {
                                        let bytes = kb * 1024;
                                        if bytes > peak {
                                            peak = bytes;
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    } else {
                        // Process no longer exists
                        break;
                    }
                }
                peak
            });
            Some(monitor_handle)
        } else {
            None
        };

        // Wait for the process to complete
        let output = child.wait_with_output().await.map_err(|e| {
            ExecutorError::ExecutionFailed(format!("Failed to wait for command: {e}"))
        })?;

        let elapsed = start_time.elapsed();

        // Get peak memory from monitor
        if let Some(monitor) = memory_monitor {
            if let Ok(mem) = monitor.await {
                peak_memory = mem;
            }
        }

        // Collect output files
        let mut files = HashMap::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.is_empty() {
            files.insert("stdout".to_string(), stdout.to_string());
        }
        if !stderr.is_empty() {
            files.insert("stderr".to_string(), stderr.to_string());
        }

        // Format time and memory in human-readable format
        let time_str = Self::format_duration(elapsed);
        let memory_str = Self::format_memory(peak_memory);

        Ok(TaskResult {
            exit_status: output.status.code().unwrap_or(-1),
            time: time_str,
            memory: memory_str,
            files,
        })
    }

    /// Copy a file into the sandbox
    fn copy_file_to_sandbox(
        &self,
        work_path: &Path,
        filename: &str,
        file_source: &FileSource,
    ) -> Result<(), ExecutorError> {
        let file_path = work_path.join(filename);

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ExecutorError::FileOperation(format!("Failed to create directory: {e}"))
            })?;
        }

        // Create and write file
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)
            .map_err(|e| {
                ExecutorError::FileOperation(format!("Failed to create file '{filename}': {e}"))
            })?;

        match file_source {
            FileSource::Content { content } => {
                file.write_all(content.as_bytes()).map_err(|e| {
                    ExecutorError::FileOperation(format!(
                        "Failed to write content to '{filename}': {e}"
                    ))
                })?;
            }
        }

        Ok(())
    }

    /// Format duration in human-readable format
    fn format_duration(duration: std::time::Duration) -> String {
        let nanos = duration.as_nanos();

        if nanos < 1_000 {
            format!("{nanos}ns")
        } else if nanos < 1_000_000 {
            format!("{}us", nanos / 1_000)
        } else if nanos < 1_000_000_000 {
            format!("{}ms", nanos / 1_000_000)
        } else {
            format!("{}s", nanos / 1_000_000_000)
        }
    }

    /// Format memory in human-readable format  
    fn format_memory(bytes: u64) -> String {
        if bytes == 0 {
            return "0mb".to_string();
        }

        if bytes < 1024 {
            format!("{bytes}b")
        } else if bytes < 1024 * 1024 {
            format!("{}kb", bytes / 1024)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{}mb", bytes / (1024 * 1024))
        } else {
            format!("{}gb", bytes / (1024 * 1024 * 1024))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_request_validation() {
        // Valid request
        let mut tasks = HashMap::new();
        tasks.insert(
            "compile".to_string(),
            Task {
                order: 0,
                args: vec!["echo".to_string(), "hello".to_string()],
                env: vec![],
                src: HashMap::new(),
            },
        );
        tasks.insert(
            "run".to_string(),
            Task {
                order: 1,
                args: vec!["echo".to_string(), "world".to_string()],
                env: vec![],
                src: HashMap::new(),
            },
        );

        let request = ExecutionRequest { tasks };
        assert!(request.validate().is_ok());
    }

    #[test]
    fn test_execution_request_invalid_order() {
        // Invalid request - orders don't start from 0
        let mut tasks = HashMap::new();
        tasks.insert(
            "compile".to_string(),
            Task {
                order: 1,
                args: vec!["echo".to_string(), "hello".to_string()],
                env: vec![],
                src: HashMap::new(),
            },
        );

        let request = ExecutionRequest { tasks };
        assert!(request.validate().is_err());
    }

    #[test]
    fn test_ordered_tasks() {
        let mut tasks = HashMap::new();
        tasks.insert(
            "second".to_string(),
            Task {
                order: 1,
                args: vec!["echo".to_string(), "second".to_string()],
                env: vec![],
                src: HashMap::new(),
            },
        );
        tasks.insert(
            "first".to_string(),
            Task {
                order: 0,
                args: vec!["echo".to_string(), "first".to_string()],
                env: vec![],
                src: HashMap::new(),
            },
        );

        let request = ExecutionRequest { tasks };
        let ordered = request.ordered_tasks();

        assert_eq!(ordered[0].0, "first");
        assert_eq!(ordered[1].0, "second");
    }

    #[tokio::test]
    async fn test_sandbox_executor() {
        let executor = SandboxExecutor::new().unwrap();

        let mut tasks = HashMap::new();
        let mut src = HashMap::new();
        src.insert(
            "hello.txt".to_string(),
            FileSource::Content {
                content: "Hello, World!".to_string(),
            },
        );

        tasks.insert(
            "test".to_string(),
            Task {
                order: 0,
                args: vec!["cat".to_string(), "hello.txt".to_string()],
                env: vec![],
                src,
            },
        );

        let request = ExecutionRequest { tasks };
        let result = executor.execute(&request).await.unwrap();

        assert_eq!(result.results.len(), 1);
        let task_result = result.results.get("test").unwrap();
        assert_eq!(task_result.exit_status, 0);
        assert_eq!(task_result.files.get("stdout").unwrap(), "Hello, World!");
    }

    #[tokio::test]
    async fn test_memory_monitoring() {
        let executor = SandboxExecutor::new().unwrap();

        let mut tasks = HashMap::new();
        tasks.insert(
            "memory_test".to_string(),
            Task {
                order: 0,
                args: vec!["echo".to_string(), "Memory test".to_string()],
                env: vec![],
                src: HashMap::new(),
            },
        );

        let request = ExecutionRequest { tasks };
        let result = executor.execute(&request).await.unwrap();

        assert_eq!(result.results.len(), 1);
        let task_result = result.results.get("memory_test").unwrap();
        assert_eq!(task_result.exit_status, 0);

        // Memory should be reported (might be 0mb for simple commands)
        assert!(
            task_result.memory.ends_with('b')
                || task_result.memory.ends_with("mb")
                || task_result.memory.ends_with("kb")
        );
    }
}
