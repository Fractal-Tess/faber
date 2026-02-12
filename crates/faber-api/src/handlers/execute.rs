use crate::{ExecutionCache, state::AppState};
use axum::{extract::State, http::StatusCode, response::Json};
use faber_runtime::{RuntimeBuilder, RuntimeResult, TaskGroup, TaskGroupResult};

pub async fn execute(
    State(app_state): State<AppState>,
    Json(task_group): Json<TaskGroup>,
) -> Result<Json<TaskGroupResult>, StatusCode> {
    if task_group.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let task_hash = ExecutionCache::generate_hash(&task_group);

    if app_state.cache_enabled {
        if let Some(cached_result) = app_state.cache.try_from_hash(&task_hash) {
            return Ok(Json(cached_result));
        }
    }

    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group.clone())
        .build();

    let result = tokio::task::spawn_blocking(move || runtime.execute())
        .await
        .map_err(|e| {
            eprintln!("Spawn blocking error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match result {
        Ok(runtime_result) => match runtime_result {
            RuntimeResult::Success(task_group_result) => {
                if app_state.cache_enabled {
                    app_state
                        .cache
                        .cache_result(task_hash, task_group_result.clone());
                }
                Ok(Json(task_group_result))
            }
            RuntimeResult::ContainerSetupFailed { error } => {
                eprintln!("Container setup failed: {}", error);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        },
        Err(e) => {
            eprintln!("Runtime execution failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
