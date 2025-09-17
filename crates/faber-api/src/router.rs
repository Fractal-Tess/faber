use axum::{Router, routing::get, routing::post};

use crate::{handlers, state::AppState};

pub fn build_router() -> Router {
    let state = AppState::new();

    Router::new()
        .route("/health", get(handlers::health))
        .route("/execute", post(handlers::execute))
        .with_state(state)
}
