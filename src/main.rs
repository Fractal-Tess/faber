use faber_api::axum;
use faber_api::{build_router, serve, ServeConfig};

mod config;
use config::Config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load configuration from environment variables
    let config = Config::from_env()?;

    let router = build_router();
    let router = axum::Router::new().nest("/api/v1", router);

    let serve_config = ServeConfig {
        port: config.port,
        host: config.host,
        router,
        max_concurrency: Some(config.max_concurrency),
    };

    serve(serve_config).await?;

    Ok(())
}
