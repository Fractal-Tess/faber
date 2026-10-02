use crate::{handlers::ErrorResponse, state::AppState};
use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();

    if a_bytes.len() != b_bytes.len() {
        return false;
    }

    let mut result: u8 = 0;
    for i in 0..a_bytes.len() {
        result |= a_bytes[i] ^ b_bytes[i];
    }
    result == 0
}

pub async fn api_key_middleware(
    State(app_state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if let Some(auth_header) = request.headers().get("Authorization")
        && let Ok(auth_str) = auth_header.to_str()
    {
        let token = auth_str.strip_prefix("Bearer ").unwrap_or(auth_str);
        // Compare against every key so the time taken does not depend on
        // which one, if any, matched.
        let accepted = app_state.api_keys.iter().fold(false, |accepted, key| {
            accepted | constant_time_eq(token, key)
        });
        if accepted {
            return next.run(request).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        Json(ErrorResponse {
            error: "Missing or invalid API key".to_string(),
        }),
    )
        .into_response()
}

static NEXT_REQUEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Tag every response with an `X-Request-Id` and log the request with it.
pub async fn request_log_middleware(request: Request<Body>, next: Next) -> Response {
    let sequence = NEXT_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let request_id = format!("{:08x}-{sequence:08x}", std::process::id());
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let started = std::time::Instant::now();

    let mut response = next.run(request).await;

    let status = response.status().as_u16();
    let elapsed_ms = started.elapsed().as_millis() as u64;
    if path.ends_with("/health") || path.ends_with("/metrics") {
        tracing::debug!(%request_id, %method, path, status, elapsed_ms, "request");
    } else {
        tracing::info!(%request_id, %method, path, status, elapsed_ms, "request");
    }
    if let Ok(value) = header::HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}
