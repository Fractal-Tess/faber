use std::net::SocketAddr;

use crate::config::FaberConfig;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::limit::GlobalConcurrencyLimitLayer;
use tracing::debug;

pub async fn serve(
    config: Arc<FaberConfig>,
    router: axum::Router,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("=== API serve function started ===");
    eprintln!("Max concurrency: {}", config.api.max_concurrency);

    let router = router.layer(GlobalConcurrencyLimitLayer::new(config.api.max_concurrency));
    eprintln!("Router configured with concurrency limit");

    let addr = SocketAddr::from((
        config.api.host.parse::<std::net::IpAddr>()?,
        config.api.port,
    ));
    eprintln!("Binding to address: {}", addr);

    let listener = TcpListener::bind(addr).await?;
    eprintln!("TCP listener bound successfully");

    debug!(
        "🦊 Faber is listening on {}:{}",
        config.api.host, config.api.port
    );
    eprintln!(
        "🦊 Faber is listening on {}:{}",
        config.api.host, config.api.port
    );

    eprintln!("Starting axum server");
    let result = axum::serve(listener, router).await;

    match &result {
        Ok(_) => eprintln!("Axum server completed successfully"),
        Err(e) => eprintln!("Axum server failed: {}", e),
    }

    eprintln!("=== API serve function completed ===");
    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
    }
}
