use axum::{Router, routing::get, routing::post};

use crate::{cache::ExecutionCache, handlers};

pub fn build_router() -> Router {
    let cache = ExecutionCache::new();

    Router::new()
        .route("/health", get(handlers::health))
        .route("/execute", post(handlers::execute))
        .with_state(cache)
}
