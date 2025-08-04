use faber_api::create_router;
use faber_config::Config;
use tracing::{error, info};

use crate::cli::Cli;

pub async fn serve(cli: Cli, graceful_shutdown: bool) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Faber...");

    // Load configuration with CLI overrides
    let config = Config::load(cli.config, cli.host, cli.port, cli.open_mode)?;

    info!("Configuration loaded successfully");
    info!("{}", config);

    let app = create_router(&config);

    let listener =
        tokio::net::TcpListener::bind(&format!("{}:{}", config.api.host, config.api.port)).await?;
    info!("🚀 Listening on {}", listener.local_addr()?);

    if graceful_shutdown {
        let shutdown_signal = async {
            tokio::signal::ctrl_c().await.ok();
        };

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await?;
    } else {
        axum::serve(listener, app).await?;
    }

    info!("Shutting down...");
    Ok(())
}

pub fn validate_config(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Validating configuration file: {}", config_path);

    match Config::from_file(config_path) {
        Ok(config) => {
            info!("✅ Configuration is valid");
            info!("{}", config);
            Ok(())
        }
        Err(e) => {
            error!("❌ Configuration validation failed: {}", e);
            Err(e.into())
        }
    }
}

pub fn show_config(
    default: bool,
    config_path: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if default {
        let config = Config::default();
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        let path = config_path.as_deref().unwrap_or("config/default.toml");
        match Config::from_file(path) {
            Ok(config) => {
                println!("{}", serde_json::to_string_pretty(&config)?);
            }
            Err(e) => {
                error!("Failed to load configuration from {}: {}", path, e);
                return Err(e.into());
            }
        }
    }
    Ok(())
}
