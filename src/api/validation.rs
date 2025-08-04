use crate::executor::Task;
use thiserror::Error;

/// Maximum number of tasks allowed per request
pub const MAX_TASKS_PER_REQUEST: usize = 100;

/// Maximum command length in characters
pub const MAX_COMMAND_LENGTH: usize = 10000;

/// Maximum environment variable value length
pub const MAX_ENV_VALUE_LENGTH: usize = 1000;

/// Maximum file content length in bytes
pub const MAX_FILE_CONTENT_LENGTH: usize = 10 * 1024 * 1024; // 10MB

/// Validation errors for API requests
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("No tasks provided")]
    NoTasks,

    #[error("Too many tasks: {0} (maximum: {1})")]
    TooManyTasks(usize, usize),

    #[error("Task at index {0}: {1}")]
    TaskError(usize, String),
}

/// Validation result
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Validate a list of tasks
pub fn validate_tasks(tasks: &[Task]) -> ValidationResult<()> {
    if tasks.is_empty() {
        return Err(ValidationError::NoTasks);
    }

    if tasks.len() > MAX_TASKS_PER_REQUEST {
        return Err(ValidationError::TooManyTasks(
            tasks.len(),
            MAX_TASKS_PER_REQUEST,
        ));
    }

    for (index, task) in tasks.iter().enumerate() {
        validate_single_task(task).map_err(|msg| ValidationError::TaskError(index, msg))?;
    }

    Ok(())
}

/// Validate a single task
fn validate_single_task(task: &Task) -> Result<(), String> {
    // Validate command
    if task.command.trim().is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    if task.command.len() > MAX_COMMAND_LENGTH {
        return Err(format!(
            "Command too long: {} characters (maximum: {})",
            task.command.len(),
            MAX_COMMAND_LENGTH
        ));
    }

    // Check for dangerous commands (basic security check)
    let dangerous_commands = [
        "rm -rf /",
        "dd if=/dev/zero",
        ":(){ :|:& };:", // Fork bomb
        "mkfs",
        "fdisk",
        "parted",
    ];

    let command_lower = task.command.to_lowercase();
    for dangerous in &dangerous_commands {
        if command_lower.contains(dangerous) {
            return Err(format!(
                "Potentially dangerous command detected: {dangerous}"
            ));
        }
    }

    // Validate arguments
    if let Some(args) = &task.args {
        for (i, arg) in args.iter().enumerate() {
            if arg.len() > MAX_COMMAND_LENGTH {
                return Err(format!(
                    "Argument {} too long: {} characters (maximum: {})",
                    i,
                    arg.len(),
                    MAX_COMMAND_LENGTH
                ));
            }
        }
    }

    // Validate environment variables
    if let Some(env) = &task.env {
        for (key, value) in env {
            if key.is_empty() {
                return Err("Environment variable key cannot be empty".to_string());
            }

            if value.len() > MAX_ENV_VALUE_LENGTH {
                return Err(format!(
                    "Environment variable '{}' value too long: {} characters (maximum: {})",
                    key,
                    value.len(),
                    MAX_ENV_VALUE_LENGTH
                ));
            }

            // Check for dangerous environment variables
            let dangerous_env_vars = ["LD_PRELOAD", "LD_LIBRARY_PATH"];
            if dangerous_env_vars.contains(&key.as_str()) {
                return Err(format!("Dangerous environment variable not allowed: {key}"));
            }
        }
    }

    // Validate files
    if let Some(files) = &task.files {
        for (path, content) in files {
            if path.is_empty() {
                return Err("File path cannot be empty".to_string());
            }

            // Check for path traversal attempts
            if path.contains("..") || path.starts_with('/') {
                return Err(format!("Invalid file path: {path}"));
            }

            if content.len() > MAX_FILE_CONTENT_LENGTH {
                return Err(format!(
                    "File '{}' too large: {} bytes (maximum: {})",
                    path,
                    content.len(),
                    MAX_FILE_CONTENT_LENGTH
                ));
            }
        }
    }

    Ok(())
}
