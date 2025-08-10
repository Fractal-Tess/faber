use std::path::PathBuf;
use std::sync::Arc;

use crate::{
    api::{RouterBuilder, serve},
    config::FaberConfig,
    logging::init_logging,
};
use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "faber-server", author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the server
    Serve {
        /// Configuration file path
        #[arg(short, long, default_value = "config/default.toml")]
        config: PathBuf,
    },
}

pub async fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let command = cli.command.unwrap_or(Commands::Serve {
        config: PathBuf::from("config/default.toml"),
    });

    match command {
        Commands::Serve { config } => {
            info!(config_path = %config.display(), "Loading configuration");
            // Load configuration
            let config = FaberConfig::load_from_path(&config)?;
            let config = Arc::new(config);

            // Initialize logging
            init_logging(Arc::clone(&config))?;

            // Build router and start the server
            let router = RouterBuilder::new(Arc::clone(&config))
                .with_public_routes()
                .with_protected_routes()
                .with_middlewares()
                .build();

            serve(config, router).await?;
            Ok(())
        }
    }
}
