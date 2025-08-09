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
    let router = router.layer(GlobalConcurrencyLimitLayer::new(config.api.max_concurrency));

    let addr = SocketAddr::from((
        config.api.host.parse::<std::net::IpAddr>()?,
        config.api.port,
    ));

    let listener = TcpListener::bind(addr).await?;

    debug!(
        "🦊 Faber is listening on {}:{}",
        config.api.host, config.api.port
    );

    axum::serve(listener, router).await?;

    Ok(())
}
