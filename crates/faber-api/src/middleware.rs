use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
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
) -> Result<Response, StatusCode> {
    if let Some(auth_header) = request.headers().get("Authorization")
        && let Ok(auth_str) = auth_header.to_str()
    {
        let token = auth_str.strip_prefix("Bearer ").unwrap_or(auth_str);
        if constant_time_eq(token, &app_state.api_key) {
            return Ok(next.run(request).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
