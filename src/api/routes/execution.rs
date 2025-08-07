use crate::api::middlewares::RequestId;
use crate::config::FaberConfig;
use axum::extract::Request;
use axum::{Extension, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info};

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    cmd: String,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    files: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

type ExecutionRequest = Vec<Task>;
type ExecutionResponse = Vec<TaskResult>;

#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Extension(request_id): Extension<RequestId>,
    Json(tasks): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<String>)> {
    debug!("Processing execution request with ID: {}", request_id);
    if tasks.is_empty() {
        debug!("Empty request");
        return Err((StatusCode::BAD_REQUEST, Json("Empty request".to_string())));
    }

    info!(
        "Starting execution for request ID: {} (max_concurrency: {})",
        request_id, config.api.max_concurrency
    );

    // INSERT_YOUR_CODE
    // Wait for 1000 seconds before proceeding (simulate long-running task)
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    info!("Completed execution for request ID: {}", request_id);

    Ok(Json(vec![]))
}
