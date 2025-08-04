use super::execution::execute_tasks;
use super::health::health_check;
use super::middleware::{auth_middleware, timing_middleware};

use axum::middleware;
use axum::{
    Extension, Router,
    routing::{get, post},
};
use faber_config::Config;
use faber_queue::QueueManager;
use std::sync::Arc;

pub fn create_router(config: &Config, queue_manager: Arc<QueueManager>) -> Router {
    let public_routes =
        Router::new().route(&config.api.endpoints.health_endpoint, get(health_check));

    let mut protected_routes = Router::new()
        .route(&config.api.endpoints.execute_endpoint, post(execute_tasks))
        .layer(Extension(queue_manager));

    protected_routes = if config.api.auth.enable.parse().unwrap_or(false) {
        protected_routes
    } else {
        protected_routes
            .layer(Extension(Arc::new(config.api.auth.secret_key.clone())))
            .layer(middleware::from_fn(auth_middleware))
    };

    let final_routes = public_routes.merge(protected_routes);

    final_routes.layer(middleware::from_fn(timing_middleware))
}
