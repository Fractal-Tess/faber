use crate::config::FaberConfig;
use axum::{Extension, Json, http::StatusCode};
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use tracing::debug;

type ExecutionRequest = Vec<Task>;
type ExecutionResponse = Vec<String>;

#[derive(Debug, Deserialize)]
pub struct Task {
    pub cmd: String,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, String>>,
}

#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Json(request): Json<ExecutionRequest>,
) -> Result<(), (StatusCode, Json<String>)> {
    debug!("Received execution request with {} tasks", request.len());
    debug!("Config: {:?}", config);

    Ok(())
}
