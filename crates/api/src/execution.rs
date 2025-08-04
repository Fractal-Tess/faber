use axum::{Json, extract::Extension, http::StatusCode};
use faber_config::Config;
use faber_core::{Task, TaskResult};
use serde::Serialize;
use std::sync::Arc;
use tracing::info;

#[derive(Serialize)]
pub struct ExecuteResponse {
    pub results: Vec<TaskResult>,
}

pub async fn execute_tasks(
    Extension(config): Extension<Arc<Config>>,
    Json(request): Json<Vec<Task>>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    info!("Received execution request with {} tasks", request.len());

    // TODO: Implement actual task execution using faber-executor
    let results = request
        .into_iter()
        .map(|task| {
            info!("Would execute task: {}", task.command);
            TaskResult {
                status: faber_core::TaskStatus::NotExecuted,
                error: Some("Task execution not yet implemented".to_string()),
                exit_code: None,
                stdout: None,
                stderr: None,
                resource_usage: faber_core::ResourceUsage::new(),
                resource_limits_exceeded: faber_core::ResourceLimitViolations::new(),
            }
        })
        .collect();

    Ok(Json(ExecuteResponse { results }))
}
