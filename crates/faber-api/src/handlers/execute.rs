use axum::{http::StatusCode, response::Json};
use faber_runtime::{Runtime, TaskGroup};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ExecuteResponse {
    message: String,
    status: String,
}

pub async fn execute(
    Json(task_group): Json<TaskGroup>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    let runtime = Runtime::new(task_group);

    let response = tokio::task::spawn_blocking(move || {
        let result = runtime.execute();

        ExecuteResponse {
            message: format!("Executed task group with result: {:?}", result),
            status: "completed".to_string(),
        }
    })
    .await
    .unwrap();

    Ok(Json(response))
}
