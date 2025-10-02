use axum::{Router, routing::get, routing::post, middleware};

use crate::{handlers, state::AppState, middleware::api_key_middleware};

pub fn build_router(api_key: String) -> Router {
    let state = AppState::new(api_key);

    Router::new()
        .route("/health", get(handlers::health))
        .route("/execute", post(handlers::execute))
        .layer(middleware::from_fn_with_state(state.clone(), api_key_middleware))
        .with_state(state)
}
