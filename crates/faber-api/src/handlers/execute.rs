use crate::{ExecutionCache, handlers::ErrorResponse, state::AppState};
use axum::{extract::State, http::StatusCode, response::Json};
use faber_runtime::{
    CgroupConfigBuilder, ExecutionStep, RuntimeBuilder, RuntimeResult, TaskGroup, TaskGroupResult,
};

pub async fn execute(
    State(app_state): State<AppState>,
    Json(mut task_group): Json<TaskGroup>,
) -> Result<Json<TaskGroupResult>, (StatusCode, Json<ErrorResponse>)> {
    if task_group.is_empty() {
        return Err(execute_error(
            StatusCode::BAD_REQUEST,
            "Task group cannot be empty",
        ));
    }
    if task_group.len() > app_state.execution_limits.max_steps
        || task_group.iter().any(|step| {
            matches!(step, ExecutionStep::Parallel(tasks) if tasks.len() > app_state.execution_limits.max_parallel_tasks)
        })
    {
        return Err(execute_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Task group exceeds configured execution limits",
        ));
    }
    for step in &mut task_group {
        let tasks = match step {
            ExecutionStep::Single(task) => std::slice::from_mut(task),
            ExecutionStep::Parallel(tasks) => tasks.as_mut_slice(),
        };
        for task in tasks {
            let profile = task
                .sandbox_profile
                .unwrap_or(app_state.execution_limits.default_sandbox_profile);
            if !app_state
                .execution_limits
                .allowed_sandbox_profiles
                .contains(&profile)
            {
                return Err(execute_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Requested sandbox profile is not allowed by service policy",
                ));
            }
            task.sandbox_profile = Some(profile);
        }
    }

    if app_state.cache_enabled {
        let task_hash = ExecutionCache::generate_hash(&task_group).map_err(|error| {
            tracing::error!(%error, "failed to serialize task group for cache key");
            execute_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Execution request failed",
            )
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
) -> Result<Json<TaskGroupResult>, (StatusCode, Json<ErrorResponse>)> {
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
        .with_overall_timeout(limits.overall_timeout)
        .build();
    let result = tokio::task::spawn_blocking(move || runtime.execute())
        .await
        .map_err(|error| {
            tracing::error!(%error, "runtime worker failed");
            execute_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Execution request failed",
            )
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
            Err(execute_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Sandbox setup failed",
            ))
        }
        Err(error) => {
            tracing::error!(%error, "runtime execution failed");
            Err(execute_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Execution request failed",
            ))
        }
    }
}

fn execute_error(status: StatusCode, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}
