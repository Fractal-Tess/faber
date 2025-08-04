use super::validation::validate_tasks;
use crate::executor::{Task, TaskExecutor, TaskResult};
use axum::{Json, http::StatusCode};
use serde::Serialize;
use tracing::{error, info};

/// Response from task execution
#[derive(Debug, Serialize)]
pub struct ExecuteTasksResponse {
    /// Execution results for each task
    pub results: Vec<TaskResult>,
    /// Total number of tasks requested
    pub total_tasks: usize,
    /// Number of successfully completed tasks
    pub successful_tasks: usize,
    /// Number of failed tasks
    pub failed_tasks: usize,
    /// Number of tasks that were not executed (due to earlier failures)
    pub skipped_tasks: usize,
}

impl ExecuteTasksResponse {
    pub fn new(results: Vec<TaskResult>) -> Self {
        let total_tasks = results.len();
        let successful_tasks = results
            .iter()
            .filter(|r| r.status == crate::executor::task::TaskStatus::Success)
            .count();
        let failed_tasks = results
            .iter()
            .filter(|r| r.error.is_some() || r.status != crate::executor::task::TaskStatus::Success)
            .count();
        let skipped_tasks = total_tasks - successful_tasks - failed_tasks;

        Self {
            results,
            total_tasks,
            successful_tasks,
            failed_tasks,
            skipped_tasks,
        }
    }
}

/// Error response for API endpoints
#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

/// Execute tasks in a secure sandboxed container
pub async fn execute_tasks(
    Json(request): Json<Vec<Task>>,
) -> Result<Json<ExecuteTasksResponse>, (StatusCode, Json<ApiErrorResponse>)> {
    // Validate tasks
    if let Err(validation_error) = validate_tasks(&request) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse {
                error: validation_error.to_string(),
                details: None,
            }),
        ));
    }

    info!("Executing {} tasks", request.len());

    // Create executor with static default configuration
    let executor = TaskExecutor::new(request);

    let executor = executor.map_err(|e| {
        error!("❌ Failed to create executor: {e:?}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse {
                error: "Failed to create task executor".to_string(),
                details: Some(e.to_string()),
            }),
        )
    })?;

    // Execute the tasks
    let results = executor.execute();

    info!("Task execution completed");

    Ok(Json(ExecuteTasksResponse::new(results)))
}
