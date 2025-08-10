use std::net::SocketAddr;

use crate::config::FaberConfig;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal::ctrl_c;
use tracing::{error, info};

pub async fn serve(
    config: Arc<FaberConfig>,
    router: axum::Router,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(host = %config.api.host, port = config.api.port, "🦊 faber starting");

    let addr = SocketAddr::from((
        config.api.host.parse::<std::net::IpAddr>()?,
        config.api.port,
    ));

    let listener = TcpListener::bind(addr).await?;

    let shutdown_signal = async {
        if let Err(e) = ctrl_c().await {
            error!("Failed to listen for shutdown signal: {e}");
        }
        info!("🦊 faber shutdown");
    };

    let result = axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal)
        .await;

    match &result {
        Ok(_) => info!("🦊 faber shutdown"),
        Err(e) => error!("🦊 faber encountered an error: {}", e),
    }

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
    }
}
