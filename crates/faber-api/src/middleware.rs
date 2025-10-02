use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    body::Body,
};
use crate::state::AppState;

pub async fn api_key_middleware(
    State(app_state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check for API key in the Authorization header
    if let Some(auth_header) = request.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            // Support "Bearer <token>" format
            if let Some(api_key) = auth_str.strip_prefix("Bearer ") {
                if api_key == app_state.api_key {
                    return Ok(next.run(request).await);
                }
            }
            // Also support direct API key
            if auth_str == app_state.api_key {
                return Ok(next.run(request).await);
            }
        }
    }

    // Check for API key in query parameters (for simpler testing)
    let uri = request.uri();
    if let Some(query) = uri.query() {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "api_key" && value == app_state.api_key {
                    return Ok(next.run(request).await);
                }
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Method, Request},
        Router,
    };
    use tower::ServiceExt;

    fn test_app() -> Router {
        let app_state = AppState::new("test-api-key".to_string());
        Router::new()
            .route("/test", axum::routing::get(|| async { "OK" }))
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                api_key_middleware,
            ))
            .with_state(app_state)
    }

    #[tokio::test]
    async fn test_api_key_bearer_token() {
        let app = test_app();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("Authorization", "Bearer test-api-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_key_direct() {
        let app = test_app();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("Authorization", "test-api-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_key_query_param() {
        let app = test_app();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test?api_key=test-api-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_invalid_api_key() {
        let app = test_app();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .header("Authorization", "Bearer wrong-api-key")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_missing_api_key() {
        let app = test_app();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}