use faber_api::axum;
use faber_api::{build_router, serve, ServeConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap_or(3000);
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    let router = build_router();

    let router = axum::Router::new().nest("/api/v1", router);

    let serve_config = ServeConfig {
        port,
        host,
        router,
        max_concurrency: Some(10),
    };

    serve(serve_config).await?;

    Ok(())
}
