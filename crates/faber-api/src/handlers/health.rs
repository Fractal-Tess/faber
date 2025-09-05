use axum::{http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ExecuteRequest {
    command: String,
    args: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

pub async fn health() -> Result<Json<HealthResponse>, StatusCode> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    Ok(Json(response))
}
