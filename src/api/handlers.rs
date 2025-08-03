use axum::{
    Extension, Json, Router, http::StatusCode, middleware, response::Response, routing::get,
    routing::post,
};
use std::sync::Arc;
use tracing::{error, info};
use utoipa::ToSchema;

use crate::api::middleware::{auth_middleware, timing_middleware};
use crate::config::ApiConfig;
use crate::executor::{ExecutionRequest, ExecutionResult, SandboxExecutor};

/// Health check response
#[derive(ToSchema, serde::Serialize)]
pub struct HealthResponse {
    /// Status message indicating API health
    status: String,
}

/// Protected endpoint response  
#[derive(ToSchema, serde::Serialize)]
pub struct ProtectedResponse {
    /// Success message indicating authenticated access
    message: String,
}

/// Error response for failed requests
#[derive(ToSchema, serde::Serialize)]
pub struct ErrorResponse {
    /// Error message describing what went wrong
    error: String,
}

pub fn create_router(config: &ApiConfig) -> Router {
    let api_key = Arc::new(config.api_key.clone());

    // Create routes that may need authentication
    let protected_routes = Router::new()
        .route("/protected", get(protected))
        .route("/run", post(run_code));

    // Apply authentication middleware only if not in OPEN mode
    let protected_routes = if config.open {
        info!("OPEN mode enabled - all routes are publicly available");
        protected_routes
    } else {
        info!("Authentication required - API key protected routes");
        protected_routes
            .layer(middleware::from_fn(auth_middleware))
            .layer(Extension(api_key.clone()))
    };

    let public_routes = Router::new().route("/health", get(health_check));

    protected_routes
        .merge(public_routes)
        .layer(middleware::from_fn(timing_middleware))
}

/// Health check endpoint - always public, returns API status
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    summary = "Check API health status",
    description = "Returns the health status of the Faber API. Always public regardless of authentication configuration.",
    responses(
        (status = 200, description = "API is healthy and operational", body = HealthResponse)
    )
)]
#[axum::debug_handler]
pub async fn health_check() -> Json<HealthResponse> {
    info!("Health check requested");
    Json(HealthResponse {
        status: "OK".to_string(),
    })
}

/// Protected demo endpoint - demonstrates authentication
#[utoipa::path(
    get,
    path = "/protected",
    tag = "protected",
    summary = "Access protected content (demo endpoint)",
    description = "Demonstrates API key authentication. Requires API key unless OPEN=true.",
    security(
        ("api_key" = [])
    ),
    responses(
        (status = 200, description = "Protected content accessed successfully", body = ProtectedResponse),
        (status = 401, description = "Unauthorized - Invalid or missing API key", body = ErrorResponse)
    )
)]
#[axum::debug_handler]
pub async fn protected() -> Json<ProtectedResponse> {
    info!("Protected route accessed");
    Json(ProtectedResponse {
        message: "Protected content accessed successfully".to_string(),
    })
}

/// Code execution endpoint - executes code in secure sandbox
#[utoipa::path(
    post,
    path = "/run",
    tag = "execution",
    summary = "Execute code in secure sandbox",
    description = "Executes code in secure sandbox with resource monitoring. Submit tasks with source files and commands.",
    request_body = ExecutionRequest,
    security(
        ("api_key" = [])
    ),
    responses(
        (status = 200, description = "Request processed successfully (check individual task status)", body = ExecutionResult),
        (status = 400, description = "Invalid request format or validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized - Invalid or missing API key", body = ErrorResponse),
        (status = 500, description = "Server error - failed to create sandbox", body = ErrorResponse)
    )
)]
#[axum::debug_handler]
pub async fn run_code(
    Json(request): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResult>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "Code execution requested with {} tasks",
        request.tasks.len()
    );

    // Validate the request - return 400 for validation errors
    if let Err(e) = request.validate() {
        error!("Invalid execution request: {e}");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid request: {e}"),
            }),
        ));
    }

    // Create a new sandbox executor for this request - return 500 for server errors
    let executor = match SandboxExecutor::new() {
        Ok(executor) => executor,
        Err(e) => {
            error!("Failed to create sandbox executor: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create sandbox: {e}"),
                }),
            ));
        }
    };

    // Execute the request - always return 200 with execution results
    match executor.execute(&request).await {
        Ok(result) => {
            info!("Code execution completed");
            Ok(Json(result))
        }
        Err(e) => {
            // This should not happen with the new design, but just in case
            error!("Unexpected execution error: {e}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Unexpected execution error: {e}"),
                }),
            ))
        }
    }
}
