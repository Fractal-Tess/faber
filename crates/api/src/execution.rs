use axum::{Json, extract::Extension, http::StatusCode};
use faber_config::Config;
use faber_core::{Task, TaskResult};
use faber_executor::TaskExecutor;
use serde::Serialize;
use std::sync::Arc;
use tracing::info;

use super::validation::validate_tasks;

#[derive(Serialize)]
pub struct ExecuteResponse {
    pub results: Vec<TaskResult>,
}

pub async fn execute_tasks(
    Extension(config): Extension<Arc<Config>>,
    Json(request): Json<Vec<Task>>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    info!("Received execution request with {} tasks", request.len());

    // Validate tasks (max 100 tasks per request)
    if let Err(validation_error) = validate_tasks(&request, 100) {
        tracing::error!("Task validation failed: {}", validation_error);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create task executor
    let executor = TaskExecutor::new((*config).clone());

    // Execute tasks
    let results = executor.execute_tasks(&request).await.map_err(|e| {
        tracing::error!("Task execution failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ExecuteResponse { results }))
}
