use crate::api::middlewares::RequestId;
use crate::config::FaberConfig;
use crate::prelude::*;
use axum::{Extension, Json, http::StatusCode};
use faber::{CgroupConfig, Mount, RuntimeBuilder, Task, TaskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    request_id: String,
    message: String,
}

// Provide a serde remote definition for the external `faber::TaskResult` type.
#[derive(Serialize)]
#[serde(remote = "TaskResult")]
struct TaskResultDef {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

// Implement Serialize for our local wrapper when it contains the remote type.
impl serde::Serialize for W<TaskResult> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TaskResultDef::serialize(&self.0, serializer)
    }
}

// Provide a serde remote definition for the external `faber::Task` type.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Task")]
struct TaskDef {
    cmd: String,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    cwd: Option<String>,
    stdin: Option<String>,
    files: Option<HashMap<String, String>>,
}

// Implement Serialize for our local wrapper when it contains the remote type.
impl serde::Serialize for W<Task> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TaskDef::serialize(&self.0, serializer)
    }
}

// Implement Deserialize for our local wrapper when it contains the remote type.
impl<'de> serde::Deserialize<'de> for W<Task> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        TaskDef::deserialize(deserializer).map(W)
    }
}

#[derive(Serialize)]
pub struct ExecutionResponse(pub Vec<W<TaskResult>>);

#[axum::debug_handler]
pub async fn execution(
    Extension(config): Extension<Arc<FaberConfig>>,
    Extension(request_id): Extension<RequestId>,
    Json(tasks): Json<Vec<W<Task>>>,
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

    let container_root = format!("{}/{}", config.container.filesystem.base_dir, request_id);
    // Use pre-parsed mounts from config; if empty, builder will use defaults
    let mounts: Vec<Mount> = config.container.filesystem.mounts.clone();

    // Build runtime via builder, applying cgroup limits if configured
    let mut builder = faber::Runtime::builder(container_root)
        .with_mounts(mounts)
        .with_workdir(config.container.filesystem.work_dir.clone());
    if let Some(cg) = &config.container.cgroups {
        builder = builder.with_cgroups(CgroupConfig {
            pids_max: cg.pids_max.clone(),
            memory_max: cg.memory_max.clone(),
            cpu_max: cg.cpu_max.clone(),
        });
    }
    let runtime = builder.build();

    let res = runtime.run(tasks.into_iter().map(|w| w.0).collect());
    debug!("Execution result: {res:?}");
    match res {
        Ok(task_result) => Ok(Json(ExecutionResponse(vec![W(task_result)]))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorPayload {
                request_id,
                message: format!("Execution failed: {e:?}"),
            }),
        )),
    }
}
