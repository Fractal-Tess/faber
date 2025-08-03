use crate::executor::{ExecutionTask, ExecutionTaskResult, Executor};
use crate::sandbox::Sandbox;
use axum::{Json, http::StatusCode};
use tracing::{debug, error};

pub async fn run(
    Json(request): Json<Vec<ExecutionTask>>,
) -> (StatusCode, Json<Vec<ExecutionTaskResult>>) {
    debug!("Request: {request:?}");

    // Create a sandbox for the request
    let sandbox = match Sandbox::new() {
        Ok(sandbox) => {
            debug!("Successfully created sandbox: {}", sandbox.sandbox_id());
            sandbox
        }
        Err(e) => {
            error!("Failed to create sandbox: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]));
        }
    };

    // Create an executor for the request
    let executor = Executor::new(request, sandbox);

    // Execute the request (executor will handle cleanup)
    let results = executor.execute();

    // Return the results
    (StatusCode::OK, Json(results))
}
