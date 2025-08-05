use clap::{CommandFactory, Parser};
use tracing::{Level, error};

use faber_cli::{Cli, Commands, init_logging, serve, validate_config};

#[tokio::main]
async fn main() {
    // Platform check - only allow Linux to run
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Error: Faber is only supported on Linux.");
        eprintln!("Current platform: {}", std::env::consts::OS);
        std::process::exit(1);
    }

    let cli = Cli::parse();

    init_logging(cli.log_level.unwrap_or(Level::INFO), &cli.log_dir).map_err(|e| {
        error!("Failed to initialize logging: {e}");
    })?;

    match cli.command {
        Some(Commands::Serve {
            log_level,
            auth_enabled,
            host,
            log_dir,
            port,
            workers,
            config,
        }) => {
            let config = GlobalConfig::load_from_path(config)?;
            serve(config).await?;
        }
        Some(Commands::Validate { config }) => {
            let config_path = config
                .as_deref()
                .or(cli.config.as_deref())
                .unwrap_or("config/config.yaml");
            validate_config(config_path)?;
        }
        Some(Commands::Config { default }) => {
            show_config(default, &cli.config)?;
        }

        None => {
            Cli::command().print_help().map_err(|e| {
                error!("Failed to print help: {e}");
            })?;
            std::process::exit(0);
        }
    };
    Ok(())
}
