use crate::api::middlewares::RequestId;
use crate::config::FaberConfig;
use crate::container::{ContainerRuntime, Task, TaskResult};
use axum::extract::Request;
use axum::{Extension, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

#[derive(Debug, Clone, Deserialize)]
pub struct TaskApi {
    cmd: String,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    files: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    request_id: String,
    message: String,
}

type ExecutionRequest = Vec<TaskApi>;
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

    let container = ContainerRuntime::new(config.container.filesystem.clone(), &request_id);
    let tasks = tasks
        .into_iter()
        .map(|t| Task {
            cmd: t.cmd,
            args: t.args,
            env: t.env,
            files: t.files,
        })
        .collect();

    let results = container.run_tasks(tasks).await;
    match results {
        Ok(results) => Ok(Json(results)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                request_id,
                message: e.to_string(),
            }),
        )),
    }
}
