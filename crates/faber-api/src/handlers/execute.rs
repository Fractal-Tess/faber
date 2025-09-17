use crate::{cache::ExecutionCache, state::AppState};
use axum::{extract::State, http::StatusCode, response::Json};
use faber_runtime::{Runtime, RuntimeResult, TaskGroup, TaskGroupResult};

pub async fn execute(
    State(app_state): State<AppState>,
    Json(task_group): Json<TaskGroup>,
) -> Result<Json<TaskGroupResult>, StatusCode> {
    // Generate hash for the task group
    let hash = ExecutionCache::generate_hash(&task_group);

    // Check cache first
    if let Some(cached_result) = app_state.cache.get(&hash) {
        return Ok(Json(cached_result));
    }

    // Execute if not in cache
    let runtime = Runtime::new(task_group);
    let result = tokio::task::spawn_blocking(move || runtime.execute())
        .await
        .unwrap();

    match result {
        Ok(runtime_result) => match runtime_result {
            RuntimeResult::Success(task_group_result) => {
                // Cache the successful result
                app_state.cache.insert(hash, task_group_result.clone());
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
