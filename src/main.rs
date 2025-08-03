use faber::api::{create_router, docs::ApiDoc};
use faber::config::ApiConfig;
use faber::logging;
use tracing::{info, warn};
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

    // Log configuration warnings
    if config.open {
        warn!("⚠️  OPEN MODE: API key authentication is DISABLED - all routes are public!");
        warn!("⚠️  This should only be used in development or for public APIs");
    } else {
        info!("🔒 Authentication enabled - API key required for protected routes");
    }

    let mut app = create_router(&config);

    // Conditionally add Swagger UI
    if config.enable_swagger {
        info!("📖 Swagger UI enabled");
        app = app
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    } else {
        info!("📖 Swagger UI disabled");
    }

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("🚀 Listening on {}", listener.local_addr().unwrap());

    if config.enable_swagger {
        info!(
            "📚 Swagger UI available at http://{}:{}/swagger-ui/",
            config.host, config.port
        );
    }

    if config.open {
        info!("🌐 All endpoints are publicly accessible (no API key required)");
    } else {
        info!("🔐 Protected endpoints require API key authentication");
    }

    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.ok();
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();

    info!("Shutting down...");
}
