use std::future::Future;
use std::pin::Pin;

use tokio::signal::unix::{SignalKind, signal};
use tracing::info;

pub type Signal = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub struct SignalBuilder {
    signal: Signal,
}

impl SignalBuilder {
    pub fn build(self) -> Signal {
        self.signal
    }
}

impl Default for SignalBuilder {
    fn default() -> Self {
        let shutdown_signal = async move {
            let sigint = tokio::signal::ctrl_c();
            tokio::pin!(sigint);
            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");

            tokio::select! {
                _ = &mut sigint => {
                }
                _ = sigterm.recv() => {
                }
            }
        };
        Self {
            signal: Box::pin(shutdown_signal),
        }
    }
}
