use axum::{routing::get, routing::post, Router};

use crate::handlers;

pub fn build_router() -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/execute", post(handlers::execute))
}
