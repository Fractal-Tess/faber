use axum::{middleware, routing::get, routing::post, Router};
use faber_store::FileStore;
use std::sync::Arc;

use crate::{handlers, middleware::api_key_middleware, state::AppState};

pub fn build_router(
    api_key: String,
    cache_enabled: bool,
    file_store: Arc<dyn FileStore>,
) -> Router {
    let state = AppState::new(api_key, cache_enabled, file_store);

    let public_routes = Router::new()
        .route("/health", get(handlers::health))
        .with_state(state.clone());

    let protected_routes = Router::new()
        .route("/execute", post(handlers::execute))
        .route(
            "/file",
            post(handlers::upload_file).get(handlers::list_files),
        )
        .route(
            "/file/{id}",
            get(handlers::download_file).delete(handlers::delete_file),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_middleware,
        ))
        .with_state(state);

    public_routes.merge(protected_routes)
}
