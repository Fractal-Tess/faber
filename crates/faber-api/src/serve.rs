use axum::Router;
use tokio::net::TcpListener;

pub struct ServeConfig {
    pub port: u16,
    pub host: String,
    pub router: Router,
    pub max_concurrency: Option<usize>,
}

pub async fn serve(config: ServeConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

    println!(
        "🦊 Faber API server listening on {}:{}",
        config.host, config.port
    );
    axum::serve(listener, config.router).await?;
    Ok(())
}
