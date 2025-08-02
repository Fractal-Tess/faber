use faber::api::{create_router, docs::ApiDoc};
use faber::config::ApiConfig;
use faber::logging;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    // Initialize logging
    logging::init_logging();

    run().await;
}

async fn run() {
    info!("Starting Faber...");

    let config = ApiConfig::new();
    info!("Configuration loaded: {}:{}", config.host, config.port);

    let app = create_router(config.api_key)
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("Listening on {}", listener.local_addr().unwrap());
    info!(
        "Swagger UI available at http://{}:{}/swagger-ui/",
        config.host, config.port
    );
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.ok();
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();

    info!("Shutting down...");
}
