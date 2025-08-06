use axum::Json;
use serde::Serialize;
use tracing::debug;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

pub async fn health() -> Json<HealthResponse> {
    debug!("Health check requested");
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}
