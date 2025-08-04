use faber_core::{FaberError, Task};
use tracing::warn;

pub fn validate_tasks(tasks: &[Task], max_tasks: usize) -> Result<(), FaberError> {
    if tasks.len() > max_tasks {
        return Err(FaberError::Validation(format!(
            "Too many tasks: {} (max: {})",
            tasks.len(),
            max_tasks
        )));
    }

    for (i, task) in tasks.iter().enumerate() {
        validate_task(task, i)?;
    }

    Ok(())
}

fn validate_task(task: &Task, index: usize) -> Result<(), FaberError> {
    if task.command.is_empty() {
        return Err(FaberError::Validation(format!(
            "Task {}: command cannot be empty",
            index
        )));
    }

    if task.command.len() > 1024 {
        return Err(FaberError::Validation(format!(
            "Task {}: command too long ({} chars, max: 1024)",
            index,
            task.command.len()
        )));
    }

    // TODO: Add more validation rules
    warn!("Task validation not fully implemented");

    Ok(())
}
