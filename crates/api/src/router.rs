use super::execution::execute_tasks;
use super::health::health_check;
use super::middleware::{auth_middleware, timing_middleware};

use axum::middleware;
use axum::{
    Extension, Router,
    routing::{get, post},
};
use faber_config::GlobalConfig;
use faber_queue::QueueManager;
use std::sync::Arc;

pub fn create_router(config: &GlobalConfig, queue_manager: Arc<QueueManager>) -> Router {
    let public_routes =
        Router::new().route(&config.api.endpoints.health_endpoint, get(health_check));

    let mut protected_routes = Router::new()
        .route(&config.api.endpoints.execute_endpoint, post(execute_tasks))
        .layer(Extension(queue_manager));

    // The config.api.auth.enable is already parsed from "env:VAR|default" format during config loading
    let auth_enabled = config.api.auth.enable.parse::<bool>().unwrap_or(false);
    if auth_enabled {
        protected_routes = protected_routes
            .layer(Extension(Arc::new(config.api.auth.secret_key.clone())))
            .layer(middleware::from_fn(auth_middleware));
    }

    let final_routes = public_routes.merge(protected_routes);

    final_routes.layer(middleware::from_fn(timing_middleware))
}
