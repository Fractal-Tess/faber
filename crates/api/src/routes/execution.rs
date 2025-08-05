use std::sync::Arc;

use axum::{Extension, Json, http::StatusCode};
use faber_config::FaberConfig;
use tracing::debug;

type ExecutionRequest = Vec<String>;
type ExecutionResponse = Vec<String>;

#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Json(request): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<String>)> {
    debug!("Received execution request with {} tasks", request.len());
    debug!("Config: {:?}", config);

    Ok(Json(vec![]))
}
