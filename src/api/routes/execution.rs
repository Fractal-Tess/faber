use crate::api::middlewares::RequestId;
use crate::config::FaberConfig;
use crate::container::ContainerRuntime;
use axum::extract::Request;
use axum::{Extension, Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, warn};

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

    let container = ContainerRuntime::new(config.container.filesystem.clone(), &request_id);

    if let Err(err) = container.prepare() {
        warn!("Failed to prepare container for {request_id}: {err}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                request_id,
                message: format!("Failed to prepare container: {err}"),
            }),
        ));
    }

    let mut results: Vec<TaskResult> = Vec::with_capacity(tasks.len());

    for task in tasks {
        if let Some(files) = &task.files {
            if let Err(err) = container.write_files(files) {
                warn!("Failed to write files for {request_id}: {err}");
                results.push(TaskResult {
                    request_id: request_id.clone(),
                    success: false,
                    stdout: String::new(),
                    stderr: format!(
                        "Failed to write files into {}: {}",
                        container.root().display(),
                        err
                    ),
                    exit_code: -1,
                });
                continue;
            }
        }

        let args = task.args.unwrap_or_default();
        let env = task.env.unwrap_or_default();

        match container.run_command(&task.cmd, &args, &env).await {
            Ok((stdout, stderr, exit_code)) => {
                results.push(TaskResult {
                    request_id: request_id.clone(),
                    success: exit_code == 0,
                    stdout,
                    stderr,
                    exit_code,
                });
            }
            Err(err) => {
                warn!("Execution error for {request_id}: {err}");
                results.push(TaskResult {
                    request_id: request_id.clone(),
                    success: false,
                    stdout: String::new(),
                    stderr: format!(
                        "Failed to execute '{}' in {}: {}",
                        task.cmd,
                        container.root().display(),
                        err
                    ),
                    exit_code: -1,
                });
            }
        }
    }

    if let Err(err) = container.cleanup() {
        warn!("Failed to cleanup container for {request_id}: {err}");
    }

    Ok(Json(results))
}
