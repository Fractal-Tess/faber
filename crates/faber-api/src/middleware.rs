use crate::state::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn api_key_middleware(
    State(app_state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(auth_header) = request.headers().get("Authorization")
        && let Ok(auth_str) = auth_header.to_str()
    {
        let token = auth_str.strip_prefix("Bearer ").unwrap_or(auth_str);
        if token == app_state.api_key {
            return Ok(next.run(request).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
