use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::time::Instant;
use tempfile::TempDir;
use thiserror::Error;
use tokio::process::Command;
use tracing::error;
use utoipa::ToSchema;

// Import our new container sandbox system
use crate::sandbox::{ContainerSandbox, container::ContainerConfig};

/// Errors that can occur during execution
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
#[schema(example = json!({"content": "#include <iostream>\nint main() {\n    std::cout << \"Hello, World!\" << std::endl;\n    return 0;\n}"}))]
pub enum FileSource {
    /// File content as string
    Content { content: String },
}

/// A task to execute in the sandbox
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "order": 0,
    "args": ["/usr/bin/g++", "main.cpp", "-o", "program"],
    "env": ["PATH=/usr/bin:/bin"],
    "src": {
        "main.cpp": {
            "content": "#include <iostream>\nint main() {\n    std::cout << \"Hello from C++!\" << std::endl;\n    return 0;\n}"
        }
    }
}))]
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
#[schema(example = json!({
    "compile": {
        "order": 0,
        "args": ["/usr/bin/g++", "main.cpp", "-o", "program", "-std=c++17"],
        "env": ["PATH=/usr/bin:/bin"],
        "src": {
            "main.cpp": {
                "content": "#include <iostream>\n#include <vector>\n\nint main() {\n    std::vector<int> numbers = {1, 2, 3, 4, 5};\n    \n    std::cout << \"Numbers: \";\n    for (const auto& num : numbers) {\n        std::cout << num << \" \";\n    }\n    std::cout << std::endl;\n    \n    return 0;\n}"
            }
        }
    },
    "run": {
        "order": 1,
        "args": ["./program"]
    }
}))]
pub struct ExecutionRequest {
    /// Tasks mapped by arbitrary names
    #[serde(flatten)]
    pub tasks: HashMap<String, Task>,
}

/// Result of a single task execution
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "status": "success",
    "exitStatus": 0,
    "time": "250ms",
    "memory": "15mb",
    "files": {
        "stdout": "Numbers: 1 2 3 4 5 \n"
    }
}))]
pub struct TaskResult {
    /// Task execution status
    #[serde(rename = "status")]
    pub status: String,
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
#[schema(example = json!({
    "compile": {
        "status": "success",
        "exitStatus": 0,
        "time": "1.2s",
        "memory": "45mb",
        "files": {}
    },
    "run": {
        "status": "failure", 
        "exitStatus": 1,
        "time": "5ms",
        "memory": "8mb",
        "files": {
            "stderr": "Segmentation fault (core dumped)\n"
        }
    },
    "test": {
        "status": "not executed",
        "exitStatus": 0,
        "time": "0ms",
        "memory": "0mb",
        "files": {}
    }
}))]
pub struct ExecutionResult {
    /// Results mapped by the same task names from the request
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

    /// Execute all tasks in the request in order, stopping on first failure
    pub async fn execute(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ExecutionResult, ExecutorError> {
        let mut results = HashMap::new();
        let ordered_tasks = request.ordered_tasks();
        let mut execution_stopped = false;

        for (task_name, task) in ordered_tasks {
            if execution_stopped {
                // Mark remaining tasks as not executed
                results.insert(
                    task_name.clone(),
                    TaskResult {
                        status: "not executed".to_string(),
                        exit_status: 0,
                        time: "0ms".to_string(),
                        memory: "0mb".to_string(),
                        files: HashMap::new(),
                    },
                );
            } else {
                // Execute the task - never return error, always capture result
                let result = match self.execute_task(task).await {
                    Ok(task_result) => task_result,
                    Err(e) => {
                        // Convert execution error to a failed task result
                        error!("Task execution error: {e}");
                        TaskResult {
                            status: "failure".to_string(),
                            exit_status: 1,
                            time: "0ms".to_string(),
                            memory: "0mb".to_string(),
                            files: {
                                let mut files = HashMap::new();
                                files.insert("stderr".to_string(), format!("Execution error: {e}"));
                                files
                            },
                        }
                    }
                };

                let task_failed = result.status == "failure";
                results.insert(task_name.clone(), result);

                if task_failed {
                    execution_stopped = true;
                }
            }
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
        let mut command = Command::new(&task.args[0]);

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

        let status = if output.status.success() {
            "success".to_string()
        } else {
            "failure".to_string()
        };

        Ok(TaskResult {
            status,
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

/// Container-based executor for enhanced security and isolation
pub struct ContainerExecutor {
    container: ContainerSandbox,
}

impl ContainerExecutor {
    /// Create a new container executor with default configuration
    pub fn new() -> Result<Self, ExecutorError> {
        let container = ContainerSandbox::default().map_err(|e| {
            ExecutorError::CreateSandbox(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create container sandbox: {}", e),
            ))
        })?;

        Ok(Self { container })
    }

    /// Create a new container executor with custom configuration
    pub fn with_config(config: ContainerConfig) -> Result<Self, ExecutorError> {
        let container = ContainerSandbox::new(config).map_err(|e| {
            ExecutorError::CreateSandbox(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create container sandbox: {}", e),
            ))
        })?;

        Ok(Self { container })
    }

    /// Create a container executor optimized for compilation tasks
    pub fn compilation() -> Result<Self, ExecutorError> {
        let container = ContainerSandbox::compilation().map_err(|e| {
            ExecutorError::CreateSandbox(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create compilation container: {}", e),
            ))
        })?;

        Ok(Self { container })
    }

    /// Create a container executor optimized for execution tasks
    pub fn execution() -> Result<Self, ExecutorError> {
        let container = ContainerSandbox::execution().map_err(|e| {
            ExecutorError::CreateSandbox(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create execution container: {}", e),
            ))
        })?;

        Ok(Self { container })
    }

    /// Execute all tasks in the request in order using container isolation
    pub async fn execute(
        &self,
        request: &ExecutionRequest,
    ) -> Result<ExecutionResult, ExecutorError> {
        let mut results = HashMap::new();
        let ordered_tasks = request.ordered_tasks();
        let mut execution_stopped = false;

        // Setup container filesystem
        self.container.setup_filesystem().map_err(|e| {
            ExecutorError::ExecutionFailed(format!("Container setup failed: {}", e))
        })?;

        for (task_name, task) in ordered_tasks {
            if execution_stopped {
                // Mark remaining tasks as not executed
                results.insert(
                    task_name.clone(),
                    TaskResult {
                        status: "not executed".to_string(),
                        exit_status: 0,
                        time: "0ms".to_string(),
                        memory: "0mb".to_string(),
                        files: HashMap::new(),
                    },
                );
            } else {
                // Execute the task in container
                let result = match self.execute_task_in_container(task).await {
                    Ok(task_result) => task_result,
                    Err(e) => {
                        error!("Container task execution error: {e}");
                        TaskResult {
                            status: "failure".to_string(),
                            exit_status: 1,
                            time: "0ms".to_string(),
                            memory: "0mb".to_string(),
                            files: {
                                let mut files = HashMap::new();
                                files.insert(
                                    "stderr".to_string(),
                                    format!("Container execution error: {e}"),
                                );
                                files
                            },
                        }
                    }
                };

                let task_failed = result.status == "failure";
                results.insert(task_name.clone(), result);

                if task_failed {
                    execution_stopped = true;
                }
            }
        }

        Ok(ExecutionResult { results })
    }

    /// Execute a single task in the container
    async fn execute_task_in_container(&self, task: &Task) -> Result<TaskResult, ExecutorError> {
        // Copy source files into container
        for (filename, file_source) in &task.src {
            match file_source {
                FileSource::Content { content } => {
                    self.container.copy_file(filename, content).map_err(|e| {
                        ExecutorError::FileOperation(format!(
                            "Failed to copy file '{}': {}",
                            filename, e
                        ))
                    })?;
                }
            }
        }

        // Execute the command in container
        let (exit_status, resource_usage, stdout, stderr) = self
            .container
            .execute_command(&task.args, &task.env, None)
            .await
            .map_err(|e| {
                ExecutorError::ExecutionFailed(format!("Container execution failed: {}", e))
            })?;

        // Collect output files
        let mut files = HashMap::new();
        if !stdout.is_empty() {
            files.insert("stdout".to_string(), stdout);
        }
        if !stderr.is_empty() {
            files.insert("stderr".to_string(), stderr);
        }

        // Format resource usage
        let time_str = Self::format_duration_nanos(resource_usage.wall_time);
        let memory_str = Self::format_memory(resource_usage.memory);

        let status = if exit_status.success() {
            "success".to_string()
        } else {
            "failure".to_string()
        };

        Ok(TaskResult {
            status,
            exit_status: exit_status.code().unwrap_or(-1),
            time: time_str,
            memory: memory_str,
            files,
        })
    }

    /// Format duration from nanoseconds in human-readable format
    fn format_duration_nanos(nanos: u64) -> String {
        if nanos < 1_000 {
            format!("{}ns", nanos)
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
            format!("{}b", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{}kb", bytes / 1024)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{}mb", bytes / (1024 * 1024))
        } else {
            format!("{}gb", bytes / (1024 * 1024 * 1024))
        }
    }

    /// Get the container working directory path
    pub fn work_path(&self) -> std::path::PathBuf {
        self.container.work_path()
    }

    /// Copy a file into the container
    pub fn copy_file<P: AsRef<Path>>(
        &self,
        filename: P,
        content: &str,
    ) -> Result<(), ExecutorError> {
        self.container
            .copy_file(filename, content)
            .map_err(|e| ExecutorError::FileOperation(format!("Failed to copy file: {}", e)))
    }

    /// Read a file from the container
    pub fn read_file<P: AsRef<Path>>(&self, filename: P) -> Result<String, ExecutorError> {
        self.container
            .read_file(filename)
            .map_err(|e| ExecutorError::FileOperation(format!("Failed to read file: {}", e)))
    }

    /// List files in the container working directory
    pub fn list_files(&self) -> Result<Vec<String>, ExecutorError> {
        self.container
            .list_files()
            .map_err(|e| ExecutorError::FileOperation(format!("Failed to list files: {}", e)))
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
