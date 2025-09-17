use axum::Router;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower::limit::GlobalConcurrencyLimitLayer;
pub struct ServeConfig {
    pub port: u16,
    pub host: String,
    pub router: Router,
    pub max_concurrency: Option<usize>,
}

pub async fn serve(config: ServeConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

    let max_concurrency = config.max_concurrency.unwrap_or(100); // Default to 100 if not specified

    println!(
        "🦊 Faber API server listening on {}:{})",
        config.host, config.port
    );

    let app = ServiceBuilder::new()
        .service(config.router)
        .layer(GlobalConcurrencyLimitLayer::new(max_concurrency));

    axum::serve(listener, app).await?;
    Ok(())
}
