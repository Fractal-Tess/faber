use faber::api::create_router;
use faber::config::Config;
use faber::logging;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Initialize logging
    logging::init_logging();

    if let Err(e) = run().await {
        error!("Application failed to start: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Faber...");

    // Load configuration from config.yaml or environment
    let config = Config::load()?;
    info!("Configuration loaded successfully");

    let app = create_router(&config);

    let listener =
        tokio::net::TcpListener::bind(&format!("{}:{}", config.server.host, config.server.port))
            .await?;
    info!("🚀 Listening on {}", listener.local_addr()?);

    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.ok();
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;

    info!("Shutting down...");
    Ok(())
}
