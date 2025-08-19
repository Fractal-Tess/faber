use crate::api::middlewares::RequestId;
use crate::config::FaberConfig;
use axum::{Extension, Json, http::StatusCode};
use faber::{Task, TaskResult};
use serde::Serialize;
use std::sync::Arc;
use tracing::debug;

use tokio::sync::{OnceCell, Semaphore};

/// Error payload returned by the API on failures.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    request_id: String,
    message: String,
}

/// Successful response containing a list of task results.
#[derive(Serialize)]
pub struct ExecutionResponse(pub Vec<TaskResult>);

static CONTAINER_SEM: OnceCell<Arc<Semaphore>> = OnceCell::const_new();

/// Execute one or more tasks inside an isolated container-like environment.
///
/// This endpoint builds a runtime using configured mounts and filesystem limits,
/// then spawns a blocking task to run the workload while respecting a global
/// semaphore to limit concurrent executions.
#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Extension(request_id): Extension<RequestId>,
    Json(tasks): Json<Vec<Task>>,
) -> Result<Json<ExecutionResponse>, (StatusCode, Json<ErrorPayload>)> {
    debug!(%request_id, task_count = tasks.len(), "execution: request start");
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
    debug!("execution: semaphore acquired");

    // Build runtime
    debug!("execution: building runtime");
    let runtime = faber::Runtime::builder()
        .with_mounts(config.container.filesystem.mounts.clone())
        .with_container_root(format!(
            "{}/{}",
            config.container.filesystem.base_dir, request_id
        ))
        .with_workdir(config.container.filesystem.work_dir.clone())
        .with_filesystem_config(
            config.container.filesystem.tmp_size.clone(),
            config.container.filesystem.workdir_size.clone(),
        )
        .with_cgroup_config(config.container.cgroup.clone().into())
        .with_kill_timeout_seconds(config.container.runtime.kill_timeout_seconds)
        .with_cpu_time_limit_ms(config.container.runtime.cpu_time_limit_ms)
        .with_task_timeout_seconds(config.container.runtime.task_timeout_seconds)
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
    debug!("execution: runtime built, spawning blocking run");

    let run_future = tokio::task::spawn_blocking(move || -> Result<Vec<TaskResult>, String> {
        // Catch panic to avoid poisoning the runtime
        match std::panic::catch_unwind(|| runtime.run(tasks)) {
            Ok(Ok(results)) => Ok(results),
            Ok(Err(e)) => Err(format!("runtime error: {e:?}")),
            Err(_) => Err("panic in runtime.run".to_string()),
        }
    });

    match run_future.await {
        Ok(Ok(results)) => {
            debug!(
                result_count = results.len(),
                "execution: run finished successfully"
            );
            Ok(Json(ExecutionResponse(results)))
        }
        Ok(Err(e)) => {
            debug!(error = %e, "execution: runtime error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    request_id,
                    message: e.to_string(),
                }),
            ))
        }
        Err(e) => {
            debug!(error = ?e, "execution: join error");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorPayload {
                    request_id,
                    message: "join error".to_string(),
                }),
            ))
        }
    }

    // Ok(Json(ExecutionResponse(results)))
}
