use std::sync::Arc;

use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};
use tracing::info;

use crate::{api::server::ServerBuilder, config::FaberConfig};

use super::{router::RouterBuilder, signal::SignalBuilder};
use tower::limit::GlobalConcurrencyLimitLayer;

pub async fn serve(config: Arc<FaberConfig>) -> Result<(), Box<dyn std::error::Error>> {
    // Main router via builder
    let router = RouterBuilder::new(Arc::clone(&config))
        .with_public_routes()
        .with_protected_routes()
        .with_middlewares()
        .build()
        .layer(GlobalConcurrencyLimitLayer::new(config.api.max_concurrency));

    let signal = SignalBuilder::default().build();

    let server = ServerBuilder::new(Arc::clone(&config))
        .with_router(router)
        .with_signal(signal);

    info!(
        "🦊 Faber is listening on {}:{}",
        config.api.host, config.api.port
    );

    server.serve().await?;

    info!("🦊 Faber is shutting down...");
    Ok(())
}
