use crate::{ExecutionCache, state::AppState};
use axum::{extract::State, http::StatusCode, response::Json};
use faber_runtime::{
    CgroupConfigBuilder, RuntimeBuilder, RuntimeResult, TaskGroup, TaskGroupResult,
};

pub async fn execute(
    State(app_state): State<AppState>,
    Json(task_group): Json<TaskGroup>,
) -> Result<Json<TaskGroupResult>, StatusCode> {
    if task_group.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    if app_state.cache_enabled {
        let task_hash = ExecutionCache::generate_hash(&task_group).map_err(|error| {
            tracing::error!(%error, "failed to serialize task group for cache key");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        if let Some(cached_result) = app_state.cache.try_from_hash(&task_hash) {
            return Ok(Json(cached_result));
        }
        return execute_uncached(
            task_group,
            app_state.execution_limits,
            Some((app_state.cache, task_hash)),
        )
        .await;
    }

    execute_uncached(task_group, app_state.execution_limits, None).await
}

async fn execute_uncached(
    task_group: TaskGroup,
    limits: crate::ExecutionLimits,
    cache: Option<(ExecutionCache, String)>,
) -> Result<Json<TaskGroupResult>, StatusCode> {
    let cgroup_config = CgroupConfigBuilder::new()
        .with_memory(limits.memory_max)
        .with_pids(limits.pids_max)
        .with_cpu(limits.cpu_max)
        .build();
    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .with_cgroup_config(cgroup_config)
        .with_timeout(limits.wall_timeout)
        .with_cpu_time_limit(limits.cpu_time_limit)
        .with_output_limit(limits.output_limit)
        .build();
    let result = tokio::task::spawn_blocking(move || runtime.execute())
        .await
        .map_err(|error| {
            tracing::error!(%error, "runtime worker failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match result {
        Ok(RuntimeResult::Success(task_group_result)) => {
            if let Some((cache, task_hash)) = cache {
                cache.cache_result(task_hash, task_group_result.clone());
            }
            Ok(Json(task_group_result))
        }
        Ok(RuntimeResult::ContainerSetupFailed { error }) => {
            tracing::error!(%error, "container setup failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
        Err(error) => {
            tracing::error!(%error, "runtime execution failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
