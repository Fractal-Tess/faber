use std::{future::IntoFuture, time::Duration};

use axum::Router;
use faber_runtime::Runtime;
use tokio::net::TcpListener;
pub struct ServeConfig {
    pub port: u16,
    pub host: String,
    pub router: Router,
    /// How long in-flight requests may take to finish after a shutdown signal.
    /// Keep it shorter than the container stop grace period (10 s for Docker).
    pub shutdown_timeout: Duration,
}

pub async fn serve(config: ServeConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

    tracing::info!(host = %config.host, port = config.port, "Faber API server listening");

    let (shutdown_started, shutdown_observed) = tokio::sync::oneshot::channel();
    let server = axum::serve(listener, config.router)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            let _ = shutdown_started.send(());
        })
        .into_future();
    tokio::pin!(server);

    tokio::select! {
        result = &mut server => result?,
        _ = async {
            if shutdown_observed.await.is_ok() {
                tokio::time::sleep(config.shutdown_timeout).await;
            } else {
                std::future::pending::<()>().await;
            }
        } => {
            tracing::warn!(
                timeout = ?config.shutdown_timeout,
                "in-flight requests did not drain before the shutdown timeout; exiting"
            );
        }
    }
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
    // Running executions notice the shutdown flag and stop their own
    // sandboxes; this also kills every request cgroup directly.
    match tokio::task::spawn_blocking(Runtime::shutdown).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::error!(%error, "failed to terminate every active sandbox"),
        Err(error) => tracing::error!(%error, "sandbox shutdown worker failed"),
    }
}
