use std::future::Future;
use std::pin::Pin;

use tokio::signal::unix::{SignalKind, signal};
use tracing::info;

pub type Signal = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>;

pub struct SignalBuilder;

impl Default for SignalBuilder {
    fn default() -> Self {
        Self
    }
}

impl SignalBuilder {
    pub fn build(self) -> Signal {
        Box::pin(async move {
            if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                let _ = sigterm.recv().await;
            } else {
                // Fallback to ctrl_c if installing SIGTERM fails
                let _ = tokio::signal::ctrl_c().await;
            }
        })
    }
}
