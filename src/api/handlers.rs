use axum::{Extension, Json, Router, middleware, routing::get, routing::post};
use std::sync::Arc;
use tracing::{error, info};
use utoipa::ToSchema;

use crate::api::middleware::{auth_middleware, timing_middleware};
use crate::config::ApiConfig;
use crate::executor::{ExecutionRequest, ExecutionResult, SandboxExecutor};

/// Health check response
#[derive(ToSchema, serde::Serialize)]
pub struct HealthResponse {
    /// Status message
    status: String,
}

/// Protected endpoint response
#[derive(ToSchema, serde::Serialize)]
pub struct ProtectedResponse {
    /// Success message
    message: String,
}

/// Error response
#[derive(ToSchema, serde::Serialize)]
pub struct ErrorResponse {
    /// Error message
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

/// Health check endpoint
///
/// Returns the health status of the API
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "API is healthy", body = HealthResponse)
    )
)]
#[axum::debug_handler]
pub async fn health_check() -> Json<HealthResponse> {
    info!("Health check requested");
    Json(HealthResponse {
        status: "OK".to_string(),
    })
}

/// Protected endpoint
///
/// Returns protected content that requires valid API key authentication (unless in OPEN mode)
#[utoipa::path(
    get,
    path = "/protected",
    tag = "protected", 
    security(
        ("api_key" = [])
    ),
    responses(
        (status = 200, description = "Protected content accessed successfully", body = ProtectedResponse),
        (status = 401, description = "Unauthorized - Invalid or missing API key (unless OPEN=true)")
    )
)]
#[axum::debug_handler]
pub async fn protected() -> Json<ProtectedResponse> {
    info!("Protected route accessed");
    Json(ProtectedResponse {
        message: "Protected content accessed successfully".to_string(),
    })
}

/// Code execution endpoint
///
/// Executes code in a secure sandbox environment with provided source files and commands
#[utoipa::path(
    post,
    path = "/run",
    tag = "execution",
    request_body = ExecutionRequest,
    security(
        ("api_key" = [])
    ),
    responses(
        (status = 200, description = "Code executed successfully", body = ExecutionResult),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 401, description = "Unauthorized - Invalid or missing API key (unless OPEN=true)"),
        (status = 500, description = "Execution failed", body = ErrorResponse)
    )
)]
#[axum::debug_handler]
pub async fn run_code(
    Json(request): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResult>, Json<ErrorResponse>> {
    info!(
        "Code execution requested with {} tasks",
        request.tasks.len()
    );

    // Validate the request
    if let Err(e) = request.validate() {
        error!("Invalid execution request: {e}");
        return Err(Json(ErrorResponse {
            error: format!("Invalid request: {e}"),
        }));
    }

    // Create a new sandbox executor for this request
    let executor = match SandboxExecutor::new() {
        Ok(executor) => executor,
        Err(e) => {
            error!("Failed to create sandbox executor: {e}");
            return Err(Json(ErrorResponse {
                error: format!("Failed to create sandbox: {e}"),
            }));
        }
    };

    // Execute the request
    match executor.execute(&request).await {
        Ok(result) => {
            info!("Code execution completed successfully");
            Ok(Json(result))
        }
        Err(e) => {
            error!("Code execution failed: {e}");
            Err(Json(ErrorResponse {
                error: format!("Execution failed: {e}"),
            }))
        }
    }
}
