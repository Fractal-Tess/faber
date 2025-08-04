use clap::{CommandFactory, Parser};
use tracing::{Level, error};

use faber_cli::{Cli, Commands, init_logging, serve, show_config, validate_config};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(
        cli.log_level.unwrap_or(Level::INFO),
        cli.debug,
        cli.log_file.as_deref(),
    );

    if let Err(e) = run(cli).await {
        error!("Application failed: {}", e);
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Some(Commands::Serve { graceful_shutdown }) => {
            serve(cli, graceful_shutdown).await?;
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
            // Show help if no subcommand is provided
            let _ = Cli::command().print_help();
            std::process::exit(0);
        }
    }
    Ok(())
}
