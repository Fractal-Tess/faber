use crate::config::FaberConfig;
use axum::{
    extract::{Extension, Request},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::{debug, warn};

// Authentication middleware to check if the request has a valid API key
pub async fn auth_middleware(
    Extension(config): Extension<Arc<FaberConfig>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    if !config.api.auth.enable {
        debug!("Auth disabled; allowing request");
        return next.run(request).await;
    }

    let api_key = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|auth_header| auth_header.strip_prefix("Bearer "))
        .or_else(|| headers.get("api_key").and_then(|value| value.to_str().ok()));

    match api_key {
        Some(api_key) => {
            if api_key != config.api.auth.api_key {
                warn!("Unauthorized request: invalid API key provided");
                return (StatusCode::UNAUTHORIZED, "Invalid API key provided").into_response();
            }
            debug!("Authorized request");
            next.run(request).await
        }
        None => (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization header or api_key header",
        )
            .into_response(),
    }
}
