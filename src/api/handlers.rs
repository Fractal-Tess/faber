use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::info;

pub async fn health_check() -> Response {
    info!("Health check requested");
    (StatusCode::OK, "OK").into_response()
}
