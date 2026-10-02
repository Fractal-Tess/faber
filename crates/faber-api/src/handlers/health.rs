use axum::{
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use faber_runtime::Runtime;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Serialize, Deserialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

/// Liveness and readiness in one: `503` once shutdown has begun, so that a
/// load balancer stops sending work to an instance that is draining.
pub async fn health() -> Response {
    let (status, text) = if Runtime::is_shutting_down() {
        (StatusCode::SERVICE_UNAVAILABLE, "shutting_down")
    } else {
        (StatusCode::OK, "ok")
    };
    let body = Json(HealthResponse {
        status: text.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    });
    (status, body).into_response()
}

/// Prometheus metrics. Behind the API key like every other non-health route.
pub async fn metrics(State(state): State<AppState>) -> Response {
    let slots_total = state.execution_limits.max_concurrency;
    let slots_in_use = slots_total.saturating_sub(state.execution_slots.available_permits());
    let body = state
        .metrics
        .render(slots_total, slots_in_use, state.cache.len());
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body).into_response()
}
