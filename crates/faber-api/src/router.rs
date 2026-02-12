use axum::{middleware, routing::get, routing::post, Router};

use crate::{handlers, middleware::api_key_middleware, state::AppState};

pub fn build_router(api_key: String, cache_enabled: bool) -> Router {
    let state = AppState::new(api_key, cache_enabled);

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
