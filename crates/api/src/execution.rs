use axum::{Json, extract::Extension, http::StatusCode};
use faber_queue::QueueManager;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::error::ApiExecutionResponseError;
use crate::types::{ApiExecutionRequest, ApiExecutionResponse};

pub async fn execute_tasks(
    Extension(queue_manager): Extension<Arc<QueueManager>>,
    Json(request): Json<ApiExecutionRequest>,
) -> Result<Json<ApiExecutionResponse>, (StatusCode, Json<ApiExecutionResponseError>)> {
    debug!("Received execution request with {} tasks", request.len());

    // Submit job to queue and wait for results
    match queue_manager.submit_job(request).await {
        Ok(results) => {
            debug!("Job completed successfully with {} results", results.len());
            Ok(Json(results))
        }
        Err(e) => {
            error!("Job execution failed: {}", e);
            let (status_code, error_msg) = match e {
                faber_queue::QueueError::QueueFull => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Queue is full. Please try again later.".to_string(),
                ),
                faber_queue::QueueError::JobTimeout {
                    job_id,
                    timeout_seconds,
                } => (
                    StatusCode::REQUEST_TIMEOUT,
                    format!("Job {job_id} timed out after {timeout_seconds} seconds"),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Internal server error: {e}"),
                ),
            };
            Err((
                status_code,
                Json(ApiExecutionResponseError { error: error_msg }),
            ))
        }
    }

    Ok(Json(vec![]))
}
