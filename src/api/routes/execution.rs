use crate::config::FaberConfig;
use crate::worker::{Task, TaskResult, WorkerPool};
use axum::{Extension, Json, http::StatusCode};
use serde::Serialize;
use std::sync::Arc;
use tracing::debug;

type ExecutionRequest = Vec<Task>;
type ExecutionResponse = Vec<TaskResult>;

#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Extension(worker_pool): Extension<Arc<tokio::sync::Mutex<WorkerPool>>>,
    Json(request): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<String>)> {
    debug!("Received execution request with {} tasks", request.len());

    if request.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json("No tasks provided".to_string()),
        ));
    }

    // Execute tasks using the worker pool
    let mut pool = worker_pool.lock().await;
    match pool.execute_tasks(request).await {
        Ok(results) => {
            debug!("Successfully executed {} tasks", results.len());
            Ok(Json(results))
        }
        Err(e) => {
            debug!("Failed to execute tasks: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(format!("Failed to execute tasks: {e}")),
            ))
        }
    }
}
