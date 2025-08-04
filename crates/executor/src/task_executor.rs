use faber_config::Config;
use faber_core::{Result, Task, TaskResult};
use tracing::info;

pub struct TaskExecutor {
    pub config: Config,
}

impl TaskExecutor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn execute_task(&self, task: &Task) -> Result<TaskResult> {
        info!("Executing task: {}", task.command);

        // TODO: Implement actual task execution using sandbox
        // For now, return a placeholder result
        Ok(TaskResult {
            status: faber_core::TaskStatus::NotExecuted,
            error: Some("Task execution not yet implemented".to_string()),
            exit_code: None,
            stdout: None,
            stderr: None,
            resource_usage: faber_core::ResourceUsage::new(),
            resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
        })
    }

    pub async fn execute_tasks(&self, tasks: &[Task]) -> Result<Vec<TaskResult>> {
        let mut results = Vec::new();

        for task in tasks {
            let result = self.execute_task(task).await?;
            results.push(result);
        }

        Ok(results)
    }
}
