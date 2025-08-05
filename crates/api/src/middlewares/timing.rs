use std::time::Instant;

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::debug;

use crate::middlewares::request_id::RequestId;

pub async fn timing_middleware(request: Request, next: Next) -> Response {
    let request_id = request.extensions().get::<RequestId>().unwrap().clone();
    let start = Instant::now();
    let mut response = next.run(request).await;
    let duration = start.elapsed();
    response.headers_mut().insert(
        "X-Response-Time",
        HeaderValue::from_str(&duration.as_secs_f64().to_string()).unwrap(),
    );
    debug!("Request {request_id:?} took {duration:?}");
    response
}
