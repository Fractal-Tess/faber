use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info};

// Authentication middleware to check if the request has a valid API key
pub async fn auth_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let api_key = headers
        .get("api_key")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            error!("Missing api_key header");
            StatusCode::UNAUTHORIZED
        })?;

    // Get the expected API key from the request extensions
    let expected_api_key = request.extensions().get::<Arc<String>>().ok_or_else(|| {
        error!("API key not configured in application");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if api_key != expected_api_key.as_str() {
        error!("Invalid API key provided");
        return Err(StatusCode::UNAUTHORIZED);
    }

    info!("API key validated successfully");
    Ok(next.run(request).await)
}

// Timing middleware to log the duration of the request
pub async fn timing_middleware(request: Request, next: Next) -> Result<Response, StatusCode> {
    let start = Instant::now();
    let response = next.run(request).await;
    let duration = start.elapsed();
    info!("Request took {:?}", duration);
    Ok(response)
}
