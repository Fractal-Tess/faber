use axum::{Json, extract::Extension, http::StatusCode};
use faber_core::{Task, TaskResult};
use faber_queue::QueueManager;
use serde::Serialize;
use std::sync::Arc;
use tracing::{error, info};

use super::validation::validate_tasks;

#[derive(Serialize)]
pub struct ExecuteResponse {
    pub results: Vec<TaskResult>,
}

pub async fn execute_tasks(
    Extension(queue_manager): Extension<Arc<QueueManager>>,
    Json(request): Json<Vec<Task>>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    info!("Received execution request with {} tasks", request.len());

    // Validate tasks (max 100 tasks per request)
    if let Err(validation_error) = validate_tasks(&request, 100) {
        error!("Task validation failed: {}", validation_error);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Submit job to queue and wait for results
    match queue_manager.submit_job(request).await {
        Ok(results) => {
            info!("Job completed successfully with {} results", results.len());
            Ok(Json(ExecuteResponse { results }))
        }
        Err(e) => {
            error!("Job execution failed: {}", e);
            match e {
                faber_queue::QueueError::QueueFull => Err(StatusCode::SERVICE_UNAVAILABLE),
                faber_queue::QueueError::JobTimeout { .. } => Err(StatusCode::REQUEST_TIMEOUT),
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
    }
}
