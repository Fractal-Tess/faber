use axum::{
    BoxError, Json, Router,
    error_handling::HandleErrorLayer,
    extract::DefaultBodyLimit,
    http::{StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::get,
    routing::post,
};
use faber_store::FileStore;
use std::sync::Arc;
use tower::{ServiceBuilder, limit::ConcurrencyLimitLayer, load_shed::LoadShedLayer};

use crate::{
    handlers,
    middleware::api_key_middleware,
    state::{AppState, ExecutionLimits},
};

pub fn build_router(
    api_key: String,
    cache_enabled: bool,
    file_store: Arc<dyn FileStore>,
    execution_limits: ExecutionLimits,
    max_concurrency: usize,
) -> Router {
    let execute_body_limit = execution_limits.execute_body_limit;
    // Multipart framing needs a small allowance beyond the configured file payload.
    let upload_body_limit = execution_limits.upload_file_limit.saturating_add(64 * 1024);
    let state = AppState::new(api_key, cache_enabled, file_store, execution_limits);

    let public_routes = Router::new()
        .route("/health", get(handlers::health))
        .with_state(state.clone());

    let protected_routes = Router::new()
        .route(
            "/execute",
            post(handlers::execute).layer(
                ServiceBuilder::new()
                    .layer(HandleErrorLayer::new(handle_execution_overload))
                    .layer(LoadShedLayer::new())
                    .layer(ConcurrencyLimitLayer::new(max_concurrency))
                    .layer(DefaultBodyLimit::max(execute_body_limit)),
            ),
        )
        .route(
            "/file",
            post(handlers::upload_file)
                .layer(DefaultBodyLimit::max(upload_body_limit))
                .get(handlers::list_files),
        )
        .route(
            "/file/{id}",
            get(handlers::download_file).delete(handlers::delete_file),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api_key_middleware,
        ))
        .with_state(state);

    public_routes.merge(protected_routes)
}

async fn handle_execution_overload(_error: BoxError) -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, "1")],
        Json(handlers::ErrorResponse {
            error: "Execution capacity is currently exhausted".to_string(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use faber_store::{StoreConfig, create_store};
    use tower::ServiceExt;

    use super::build_router;
    use crate::ExecutionLimits;

    fn multipart_body(size: usize) -> Vec<u8> {
        let mut body = b"--faber\r\nContent-Disposition: form-data; name=\"file\"; filename=\"large.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n".to_vec();
        body.resize(body.len() + size, b'x');
        body.extend_from_slice(b"\r\n--faber--\r\n");
        body
    }

    async fn upload(size: usize, file_limit: usize) -> StatusCode {
        let router = build_router(
            "test-key".to_string(),
            false,
            create_store(StoreConfig::builder().max_file_size(file_limit as u64).build()),
            ExecutionLimits {
                upload_file_limit: file_limit,
                ..ExecutionLimits::default()
            },
            10,
        );
        let request = Request::post("/file")
            .header("Authorization", "Bearer test-key")
            .header("Content-Type", "multipart/form-data; boundary=faber")
            .body(Body::from(multipart_body(size)))
            .unwrap();
        router.oneshot(request).await.unwrap().status()
    }

    #[tokio::test]
    async fn upload_above_axum_default_is_accepted() {
        assert_eq!(upload(3 * 1024 * 1024, 4 * 1024 * 1024).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn upload_above_configured_file_limit_is_rejected() {
        assert_eq!(upload(2048, 1024).await, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn saturated_execute_is_shed_without_blocking_health() {
        let router = build_router(
            "test-key".to_string(),
            false,
            create_store(StoreConfig::default()),
            ExecutionLimits::default(),
            0,
        );
        let execute = Request::post("/execute")
            .header("Authorization", "Bearer test-key")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"[{"cmd":"/bin/true"}]"#))
            .unwrap();
        let response = router.clone().oneshot(execute).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers().get("retry-after").unwrap(), "1");

        let health = router
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(health.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn execute_validation_errors_use_json_shape() {
        let router = build_router(
            "test-key".to_string(),
            false,
            create_store(StoreConfig::default()),
            ExecutionLimits::default(),
            1,
        );
        let request = Request::post("/execute")
            .header("Authorization", "Bearer test-key")
            .header("Content-Type", "application/json")
            .body(Body::from("[]"))
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({"error": "Task group cannot be empty"})
        );
    }
}
