use crate::executor::{ExecutionTask, ExecutionTaskResult, Executor};
use axum::{Json, http::StatusCode};
use tracing::{debug, error};

pub async fn run(
    Json(request): Json<Vec<ExecutionTask>>,
) -> (StatusCode, Json<Vec<ExecutionTaskResult>>) {
    debug!("Request: {request:?}");

    // Create an executor with secure container sandbox
    let executor = match Executor::new(request) {
        Ok(executor) => {
            debug!("Successfully created executor with secure container");
            executor
        }
        Err(e) => {
            error!("Failed to create executor: {e:?}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]));
        }
    };

    // Execute the request (executor manages container lifecycle)
    let results = executor.execute();

    // Return the results
    (StatusCode::OK, Json(results))
}
