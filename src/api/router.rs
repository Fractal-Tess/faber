use std::sync::Arc;

use super::middlewares::{auth_middleware, request_id_middleware, timing_middleware};
use super::routes::{execution, health};
use crate::config::FaberConfig;
use crate::executor::ExecutorPool;

use axum::middleware;
use axum::{
    Extension, Router,
    routing::{get, post},
};

pub async fn create_router(config: Arc<FaberConfig>) -> Router {
    let config_extension = Extension(Arc::clone(&config));

    let executor_pool = ExecutorPool::new(Arc::clone(&config))
        .await
        .expect("Failed to create executor pool");
    let executor_pool_extension = Extension(Arc::new(tokio::sync::Mutex::new(executor_pool)));

    let public_routes = Router::new().route(&config.api.endpoints.health_endpoint, get(health));

    let protected_routes = Router::new().route(
        &config.api.endpoints.task_execution_endpoint,
        post(execution),
    );

    let final_routes = public_routes.merge(protected_routes);

    final_routes
        .layer(middleware::from_fn(timing_middleware))
        .layer(middleware::from_fn(auth_middleware))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(config_extension)
        .layer(executor_pool_extension)
}
