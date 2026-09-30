use crate::{ExecutionCache, handlers::ErrorResponse, state::AppState};
use axum::{
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use faber_runtime::{
    CancellationToken, CgroupConfigBuilder, ExecutionStep, FaberError, Runtime, RuntimeBuilder,
    RuntimeResult, TaskGroup, TaskGroupResult,
};
use tokio::sync::OwnedSemaphorePermit;

/// JSON error returned by `/execute`.
#[derive(Debug)]
pub struct ExecuteError {
    status: StatusCode,
    message: String,
    retry_after: Option<u32>,
}

impl ExecuteError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl IntoResponse for ExecuteError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        match self.retry_after {
            Some(seconds) => (
                self.status,
                [(header::RETRY_AFTER, seconds.to_string())],
                body,
            )
                .into_response(),
            None => (self.status, body).into_response(),
        }
    }
}

/// Cancels the sandbox when the handler future is dropped, which happens when
/// the client disconnects. Cancelling a finished execution is a no-op.
struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub async fn execute(
    State(app_state): State<AppState>,
    Json(mut task_group): Json<TaskGroup>,
) -> Result<Json<TaskGroupResult>, ExecuteError> {
    if task_group.is_empty() {
        return Err(ExecuteError::new(
            StatusCode::BAD_REQUEST,
            "Task group cannot be empty",
        ));
    }
    if task_group.len() > app_state.execution_limits.max_steps
        || task_group.iter().any(|step| {
            matches!(step, ExecutionStep::Parallel(tasks) if tasks.len() > app_state.execution_limits.max_parallel_tasks)
        })
    {
        return Err(ExecuteError::new(
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
                return Err(ExecuteError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Requested sandbox profile is not allowed by service policy",
                ));
            }
            task.sandbox_profile = Some(profile);
        }
    }

    let cache_key = if app_state.cache_enabled {
        let task_hash = ExecutionCache::generate_hash(&task_group).map_err(|error| {
            tracing::error!(%error, "failed to serialize task group for cache key");
            ExecuteError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Execution request failed",
            )
        })?;
        if let Some(cached_result) = app_state.cache.try_from_hash(&task_hash) {
            return Ok(Json(cached_result));
        }
        Some((app_state.cache.clone(), task_hash))
    } else {
        None
    };

    // One slot per task that can run at once; the aggregate cgroup limits are
    // sized as per-task limits times the number of slots.
    let slots = u32::try_from(Runtime::widest_step(&task_group)).unwrap_or(u32::MAX);
    let permit = app_state
        .execution_slots
        .clone()
        .try_acquire_many_owned(slots)
        .map_err(|_| ExecuteError {
            retry_after: Some(1),
            ..ExecuteError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Execution capacity is currently exhausted",
            )
        })?;

    execute_uncached(task_group, app_state.execution_limits, cache_key, permit).await
}

async fn execute_uncached(
    task_group: TaskGroup,
    limits: crate::ExecutionLimits,
    cache: Option<(ExecutionCache, String)>,
    permit: OwnedSemaphorePermit,
) -> Result<Json<TaskGroupResult>, ExecuteError> {
    let cgroup_config = CgroupConfigBuilder::new()
        .with_memory(limits.memory_max)
        .with_pids(limits.pids_max)
        .with_cpu(limits.cpu_max)
        .build();
    let cancellation = CancellationToken::new();
    let _cancel_on_drop = CancelOnDrop(cancellation.clone());
    let runtime = RuntimeBuilder::default()
        .with_task_group(task_group)
        .with_cgroup_config(cgroup_config)
        .with_timeout(limits.wall_timeout)
        .with_cpu_time_limit(limits.cpu_time_limit)
        .with_output_limit(limits.output_limit)
        .with_overall_timeout(limits.overall_timeout)
        .with_cancellation(cancellation)
        .build();
    // The permit moves into the blocking closure so the slot stays taken
    // until the sandbox has really finished, even if this future is dropped.
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        runtime.execute()
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "runtime worker failed");
        ExecuteError::new(
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
            Err(ExecuteError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Sandbox setup failed",
            ))
        }
        Err(FaberError::TaskTimeout {
            timeout_duration, ..
        }) => {
            tracing::error!(?timeout_duration, "execution overran its overall deadline");
            Err(ExecuteError::new(
                StatusCode::GATEWAY_TIMEOUT,
                format!(
                    "Execution exceeded the overall deadline of {} ms and was killed",
                    timeout_duration.as_millis()
                ),
            ))
        }
        Err(FaberError::ShuttingDown) => Err(ExecuteError {
            retry_after: Some(1),
            ..ExecuteError::new(StatusCode::SERVICE_UNAVAILABLE, "Faber is shutting down")
        }),
        Err(FaberError::Cancelled) => {
            tracing::info!("execution cancelled");
            Err(ExecuteError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "Execution was cancelled",
            ))
        }
        Err(error) => {
            tracing::error!(%error, "runtime execution failed");
            Err(ExecuteError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Execution request failed",
            ))
        }
    }
}
