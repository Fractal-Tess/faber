use crate::api::middlewares::RequestId;
use crate::config::FaberConfig;
use axum::{Extension, Json, http::StatusCode};
use faber::{Task, TaskResult};
use serde::Serialize;
use std::sync::Arc;
use tracing::debug;

use tokio::sync::{OnceCell, Semaphore};

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    request_id: String,
    message: String,
}

#[derive(Serialize)]
pub struct ExecutionResponse(pub Vec<TaskResult>);

static CONTAINER_SEM: OnceCell<Arc<Semaphore>> = OnceCell::const_new();

#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Extension(request_id): Extension<RequestId>,
    Json(tasks): Json<Vec<Task>>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<ErrorPayload>)> {
    if tasks.is_empty() {
        debug!("Request {request_id} is empty");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorPayload {
                request_id,
                message: "Empty request".to_string(),
            }),
        ));
    }

    debug!(
        task_count = tasks.len(),
        "Building runtime and acquiring semaphore"
    );

    // Initialize (or fetch) global semaphore to throttle container runs
    let sem = CONTAINER_SEM
        .get_or_init(|| async { Arc::new(Semaphore::new(config.api.max_concurrency.max(1))) })
        .await
        .clone();

    let _permit = sem.acquire().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                request_id: request_id.clone(),
                message: format!("semaphore closed: {e}"),
            }),
        )
    })?;

    // Build runtime
    let runtime = faber::Runtime::builder()
        .with_mounts(config.container.filesystem.mounts.clone())
        .with_container_root(format!(
            "{}/{}",
            config.container.filesystem.base_dir, request_id
        ))
        .with_workdir(config.container.filesystem.work_dir.clone())
        .with_runtime_limits(faber::RuntimeLimits {
            kill_timeout_seconds: config.container.runtime.kill_timeout_seconds,
        })
        .with_id(request_id.clone())
        .build()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    request_id: request_id.clone(),
                    message: format!("Failed to build runtime: {e}"),
                }),
            )
        })?;

    debug!("Spawning blocking runtime.run");

    let run_future = tokio::task::spawn_blocking(move || -> Result<Vec<TaskResult>, String> {
        // Catch panic to avoid poisoning the runtime
        match std::panic::catch_unwind(|| runtime.run(tasks)) {
            Ok(Ok(results)) => Ok(results),
            Ok(Err(e)) => Err(format!("runtime error: {e:?}")),
            Err(_) => Err("panic in runtime.run".to_string()),
        }
    });

    match run_future.await {
        Ok(Ok(results)) => Ok(Json(ExecutionResponse(results))),
        Ok(Err(e)) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                request_id,
                message: e.to_string(),
            }),
        )),
        Err(_) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                request_id,
                message: "join error".to_string(),
            }),
        )),
    }

    // Ok(Json(ExecutionResponse(results)))
}
