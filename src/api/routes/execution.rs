use crate::api::middlewares::RequestId;
use crate::config::FaberConfig;
use crate::sandbox::container::{ContainerError, ContainerRuntime};
use axum::extract::Request;
use axum::{Extension, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    cmd: String,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    files: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    request_id: String,
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    request_id: String,
    message: String,
}

type ExecutionRequest = Vec<Task>;
type ExecutionResponse = Vec<TaskResult>;

#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Extension(request_id): Extension<RequestId>,
    Json(tasks): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<ErrorPayload>)> {
    if tasks.is_empty() {
        debug!("Requst {request_id} is empty");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorPayload {
                request_id,
                message: "Empty request".to_string(),
            }),
        ));
    }

    info!(
        "Starting execution for request ID: {} (max_concurrency: {})",
        request_id, config.api.max_concurrency
    );

    let container = ContainerRuntime::new(config.container.filesystem.clone(), &request_id);

    if let Err(err) = container.prepare() {
        warn!("Failed to prepare container for {request_id}: {err}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                request_id,
                message: format!(
                    "Failed to prepare container at {}: {}",
                    container.root().display(),
                    err
                ),
            }),
        ));
    }

    // Simulate execution work (placeholder)
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    if let Err(err) = container.cleanup() {
        warn!("Failed to cleanup container for {request_id}: {err}");
        // Even on cleanup failure, we still return success for the executed task in this phase.
    }

    Ok(Json(vec![TaskResult {
        request_id,
        success: true,
        stdout: "".to_string(),
        stderr: "".to_string(),
        exit_code: 0,
    }]))
}
