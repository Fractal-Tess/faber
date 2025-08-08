use std::sync::Arc;

use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};
use tracing::info;

use crate::config::FaberConfig;

use super::router::RouterBuilder;
use tower::limit::GlobalConcurrencyLimitLayer;

pub async fn serve(config: Arc<FaberConfig>) -> Result<(), Box<dyn std::error::Error>> {
    // Main router via builder
    let router = RouterBuilder::new(Arc::clone(&config))
        .with_public_routes()
        .with_protected_routes()
        .with_middlewares()
        .build()
        .layer(GlobalConcurrencyLimitLayer::new(config.api.max_concurrency));

    // Shutdown signal
    let shutdown_signal = async move {
        let sigint = tokio::signal::ctrl_c();
        tokio::pin!(sigint);
        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");

        tokio::select! {
            _ = &mut sigint => {
                info!("Shutdown signal (SIGINT/Ctrl+C) received, stopping server...");
            }
            _ = sigterm.recv() => {
                info!("Shutdown signal (SIGTERM/Docker exit) received, stopping server...");
            }
        }
    };

    let listener = TcpListener::bind(&format!("{}:{}", config.api.host, config.api.port)).await?;

    info!("🦊 Faber is listening on {}", listener.local_addr()?);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    info!("🦊 Faber is shutting down...");
    Ok(())
}
