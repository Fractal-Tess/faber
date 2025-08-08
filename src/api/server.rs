use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;
use tracing::info;

use crate::config::FaberConfig;

use super::signal::Signal;

pub struct ServerBuilder {
    config: Arc<FaberConfig>,
    router: Option<Router>,
    signal: Option<Signal>,
}

impl ServerBuilder {
    pub fn new(config: Arc<FaberConfig>) -> Self {
        Self {
            config,
            router: None,
            signal: None,
        }
    }

    pub fn with_signal(mut self, signal: Signal) -> Self {
        self.signal = Some(signal);
        self
    }

    pub fn with_router(mut self, new_router: Router) -> Self {
        // If there is a router, merge it with the new router
        if let Some(router) = self.router.take() {
            let router = router.merge(new_router);
            self.router = Some(router);
        } else {
            self.router = Some(new_router);
        }

        self
    }

    pub async fn serve(self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&format!(
            "{}:{}",
            self.config.api.host, self.config.api.port
        ))
        .await?;

        let app = self.router.expect("router must be set with with_router()");

        // Use provided shutdown signal or a default ctrl_c future
        let shutdown = self.signal.unwrap_or_else(|| {
            Box::pin(async {
                let _ = tokio::signal::ctrl_c().await;
            })
        });

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await?;

        Ok(())
    }
}
