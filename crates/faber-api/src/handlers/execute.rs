use axum::{http::StatusCode, response::Json};
use faber_runtime::{Runtime, RuntimeResult, TaskGroup, TaskGroupResult};

pub async fn execute(
    Json(task_group): Json<TaskGroup>,
) -> Result<Json<TaskGroupResult>, StatusCode> {
    let runtime = Runtime::new(task_group);

    let result = tokio::task::spawn_blocking(move || runtime.execute())
        .await
        .unwrap();

    match result {
        Ok(runtime_result) => match runtime_result {
            RuntimeResult::Success(task_group_result) => Ok(Json(task_group_result)),
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
