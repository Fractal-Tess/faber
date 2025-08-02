use axum::{
    Router,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use tracing::info;

use crate::api::middleware::{auth_middleware, timing_middleware};

pub fn create_router() -> Router {
    let protected_routes = Router::new()
        .route("/protected", get(protected))
        .layer(middleware::from_fn(auth_middleware));

    let public_routes = Router::new().route("/health", get(health_check));

    protected_routes
        .merge(public_routes)
        .layer(middleware::from_fn(timing_middleware))
}

async fn health_check() -> Response {
    info!("Health check requested");
    (StatusCode::OK, "OK").into_response()
}

async fn protected() -> Response {
    info!("Protected route accessed with valid API key");
    (StatusCode::OK, "Protected content accessed successfully").into_response()
}
