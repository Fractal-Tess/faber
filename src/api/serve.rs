use std::sync::Arc;

use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};
use tracing::info;

use crate::config::FaberConfig;

use super::create_router;

pub async fn serve(config: Arc<FaberConfig>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting 🦊 Faber... ");

    let app = create_router(Arc::clone(&config)).await;

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

    info!("🚀 Listening on {}", listener.local_addr()?);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    info!("Shutting down...");
    Ok(())
}
