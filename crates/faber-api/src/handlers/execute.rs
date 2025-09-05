use axum::{http::StatusCode, response::Json};
use faber_runtime::TaskGroup;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ExecuteResponse {
    message: String,
    status: String,
}

pub async fn execute(
    Json(task_group): Json<TaskGroup>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    println!("Received TaskGroup with  total tasks",);

    let response = ExecuteResponse {
        message: format!(
            "Received TaskGroup with {} execution steps",
            task_group.len()
        ),
        status: "accepted".to_string(),
    };

    Ok(Json(response))
}
