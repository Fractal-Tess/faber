use std::sync::Arc;

use super::middlewares::{auth_middleware, request_id_middleware, timing_middleware};
use super::routes::{execution, health};
use crate::config::FaberConfig;

use axum::middleware;
use axum::{
    Extension, Router,
    routing::{get, post},
};
use tokio::sync::Semaphore;
use tower::limit::GlobalConcurrencyLimitLayer;

pub async fn create_router(config: Arc<FaberConfig>) -> Router {
    let config_extension = Extension(Arc::clone(&config));

    let public_routes = Router::new().route(&config.api.endpoints.health_endpoint, get(health));

    let protected_routes = Router::new().route(
        &config.api.endpoints.task_execution_endpoint,
        post(execution),
    );

    let final_routes = public_routes.merge(protected_routes);

    final_routes
        .layer(middleware::from_fn(auth_middleware))
        .layer(middleware::from_fn(timing_middleware))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(config_extension)
    // .layer(GlobalConcurrencyLimitLayer::new(config.api.max_concurrency))
}
