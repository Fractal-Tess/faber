use axum::{Router, middleware, routing::get, routing::post};

use crate::{handlers, middleware::api_key_middleware, state::AppState};

pub fn build_router(api_key: String) -> Router {
    let state = AppState::new(api_key);

    let public_routes = Router::new()
        .route("/health", get(handlers::health))
        .with_state(state.clone());

    let protected_routes = Router::new()
        .route("/execute", post(handlers::execute))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_middleware,
        ))
        .with_state(state);

    public_routes.merge(protected_routes)
}
