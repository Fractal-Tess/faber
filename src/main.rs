use faber::api::create_router;
use faber::config::Config;
use faber::logging;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize logging
    logging::init_logging();

    run().await;
}

async fn run() {
    info!("Starting Faber...");

    let config = Config::from_env();

    let app = create_router(&config);

    let listener = tokio::net::TcpListener::bind(&format!("{}:{}", config.host, config.port))
        .await
        .unwrap();
    info!("🚀 Listening on {}", listener.local_addr().unwrap());

    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.ok();
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();

    info!("Shutting down...");
}
