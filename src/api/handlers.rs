use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub async fn health_check() -> Response {
    (StatusCode::OK, "OK").into_response()
}
