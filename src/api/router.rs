use super::health::health_check;
use super::middleware::{auth_middleware, timing_middleware};
use super::run::run;

use crate::config::Config;
use axum::middleware;
use axum::{
    Router,
    routing::{get, post},
};
use tracing::debug;

pub fn create_router(config: &Config) -> Router {
    debug!("Configuration loaded: {config}");

    let public_routes = Router::new().route("/health", get(health_check));

    let mut protected_routes = Router::new().route("/run", post(run));

    protected_routes = if config.open {
        protected_routes
    } else {
        protected_routes.layer(middleware::from_fn(auth_middleware))
    };

    let final_routes = public_routes.merge(protected_routes);

    final_routes.layer(middleware::from_fn(timing_middleware))
}
