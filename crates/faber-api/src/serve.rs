use axum::Router;
use faber_runtime::Runtime;
use tokio::net::TcpListener;
pub struct ServeConfig {
    pub port: u16,
    pub host: String,
    pub router: Router,
}

pub async fn serve(config: ServeConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

    tracing::info!(host = %config.host, port = config.port, "Faber API server listening");

    axum::serve(listener, config.router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate()).ok();
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(error) = result {
                    tracing::error!(%error, "failed to install Ctrl-C handler");
                }
            }
            _ = async {
                if let Some(signal) = terminate.as_mut() {
                    signal.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }

    #[cfg(not(unix))]
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install Ctrl-C handler");
    }

    tracing::info!("shutdown requested; terminating active sandboxes");
    if let Err(error) = Runtime::shutdown() {
        tracing::error!(%error, "failed to terminate every active sandbox");
    }
}
