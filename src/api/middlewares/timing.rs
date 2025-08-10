use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Instant;
use tracing::{debug, warn};

use crate::api::middlewares::RequestId;

pub async fn timing_middleware(mut request: Request, next: Next) -> Result<Response, Response> {
    // Ensure a RequestId exists; generate one if missing
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .cloned()
        .unwrap_or_else(|| {
            let id: RequestId = uuid::Uuid::new_v4().to_string();
            id
        });

    // Insert the request_id for downstream handlers
    request.extensions_mut().insert(request_id.clone());

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
