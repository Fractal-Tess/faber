use std::time::Instant;

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::{debug, info, warn};

use crate::middlewares::request_id::RequestId;

pub async fn timing_middleware(request: Request, next: Next) -> Response {
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

    let request_id = response.extensions().get::<RequestId>();

    // Log the timing information with request ID if available
    match request_id {
        Some(id) => debug!("Request {id:?} took {duration:?}"),
        None => debug!("Request took {duration:?} (no request ID available)"),
    }

    response
}
