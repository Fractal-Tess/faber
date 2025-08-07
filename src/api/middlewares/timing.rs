use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Instant;
use tracing::{debug, warn};

use crate::api::middlewares::request_id::RequestId;

pub async fn timing_middleware(request: Request, next: Next) -> Result<Response, Response> {
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .cloned()
        .ok_or_else(|| {
            warn!("===Request ID not found - this should not happen===");
            (StatusCode::INTERNAL_SERVER_ERROR, "Request ID not found").into_response()
        })?;

    debug!("=== Request {request_id:?} started ===");

    let start = Instant::now();
    let mut response = next.run(request).await;
    let duration = start.elapsed();

    // Try to add the response time header, log warning if it fails
    if let Ok(header_value) = HeaderValue::from_str(format!("{}ms", duration.as_millis()).as_str())
    {
        response
            .headers_mut()
            .insert("X-Response-Time", header_value);
    } else {
        warn!("Failed to create X-Response-Time header value");
    }

    debug!("=== Request {request_id:?} took {duration:?} ===");

    Ok(response)
}
