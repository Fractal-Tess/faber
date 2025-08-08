use std::sync::Arc;

use super::middlewares::{auth_middleware, request_id_middleware, timing_middleware};
use super::routes::{execution, health};
use crate::config::FaberConfig;

use axum::middleware;
use axum::routing::Route;
use axum::{
    Extension, Router,
    routing::{get, post},
};

pub struct RouterBuilder {
    config: Arc<FaberConfig>,
    router: Router,
}

impl RouterBuilder {
    pub fn new(config: Arc<FaberConfig>) -> Self {
        Self {
            config,
            router: Router::new(),
        }
    }

    pub fn with_public_routes(mut self) -> Self {
        let route = get(health);
        self.router = self
            .router
            .route(&self.config.api.endpoints.health_endpoint, route);
        self
    }

    pub fn with_protected_routes(mut self) -> Self {
        let route = post(execution);
        self.router = self
            .router
            .route(&self.config.api.endpoints.task_execution_endpoint, route);
        self
    }

    pub fn with_middlewares(mut self) -> Self {
        let config_extension = Extension(Arc::clone(&self.config));
        self.router = self
            .router
            .layer(middleware::from_fn(auth_middleware))
            .layer(middleware::from_fn(timing_middleware))
            .layer(middleware::from_fn(request_id_middleware))
            .layer(config_extension);
        self
    }

    pub fn build(self) -> Router {
        self.router
    }
}
