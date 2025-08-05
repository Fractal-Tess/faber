use faber_api::create_router;
use faber_config::GlobalConfig;
use faber_queue::QueueManager;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal::unix::{SignalKind, signal};
use tracing::{error, info};

use crate::cli::Cli;

pub async fn serve(config: GlobalConfig) -> Result<(), Box<dyn std::error::Error>> {
    info!("🦊 Starting Faber  api server... ");

    let queue_manager = Arc::new(QueueManager::new(config.queue.clone()));

    let app = create_router(&config, Arc::clone(&queue_manager));

    let queue_manager_shutdown = Arc::clone(&queue_manager);
    let shutdown_signal = async move {
        let mut sigint = tokio::signal::ctrl_c();
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

        // Shutdown the queue manager
        info!("Shutting down queue system...");
        if let Err(e) = queue_manager_shutdown.shutdown().await {
            error!("Failed to shutdown queue manager gracefully: {}", e);
        } else {
            info!("Queue system shutdown complete");
        }
    };

    let listener = TcpListener::bind(&format!("{}:{}", config.api.host, config.api.port)).await?;

    info!("🚀 Listening on {}", listener.local_addr()?);
    // Run the server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    axum::serve(listener, app).await?;

    // Shutdown the queue manager
    info!("Shutting down queue system...");
    if let Err(e) = queue_manager.shutdown().await {
        error!("Failed to shutdown queue manager: {}", e);
    }
    info!("Queue system shutdown complete");

    info!("Shutting down...");
    Ok(())
}

pub fn validate_config(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Validating configuration file: {}", config_path);

    match GlobalConfig::load_from_path(config_path) {
        Ok(config) => {
            info!("✅ Configuration is valid");
            info!("{}", config);
            Ok(())
        }
        Err(e) => {
            error!("❌ Configuration validation failed: {}", e);
            Err(e.into())
        }
    }
}
