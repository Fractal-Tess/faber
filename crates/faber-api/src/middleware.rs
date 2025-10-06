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
        if let Some(api_key) = auth_str.strip_prefix("Bearer ")
            && api_key == app_state.api_key
        {
            return Ok(next.run(request).await);
        }
        if auth_str == app_state.api_key {
            return Ok(next.run(request).await);
        }
    }
    let uri = request.uri();
    if let Some(query) = uri.query() {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=')
                && key == "api_key"
                && value == app_state.api_key
            {
                return Ok(next.run(request).await);
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}
