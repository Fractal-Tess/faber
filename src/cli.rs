use std::sync::Arc;

use crate::config::FaberConfig;
use crate::{api::serve, logging::init_logging};
use std::error::Error;
use std::process::exit;
use tracing::{error, info};

use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "faber")]
#[command(about = "A secure containerized task execution service")]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    #[arg(short, long, default_value = "/faber/config/default.toml")]
    pub config: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the Faber server
    Serve {
        /// Configuration file path
        #[arg(short, long, default_value = "config/default.toml")]
        config: String,
    },
    /// Validate configuration, optionally display the parsed config
    ValidateConfig {
        /// Configuration file path
        #[arg(short, long, default_value = "config/default.toml")]
        config: String,
        /// Display the parsed configuration after validation
        #[arg(short, long)]
        display: bool,
    },
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Platform check - only allow Linux to run
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Error: Faber is only supported on Linux.");
        eprintln!("Current platform: {}", std::env::consts::OS);
        exit(1);
    }

    let cli = Cli::parse();

    let command = cli.command.unwrap_or_else(|| {
        Cli::command()
            .print_help()
            .map_err(|e| {
                eprintln!("Failed to print help: {e}");
            })
            .expect("Failed to print help");
        exit(0);
    });

    match command {
        Commands::Serve { config } => {
            let config = match FaberConfig::load_from_path(&config) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Failed to load configuration from {}: {}", config, e);
                    exit(1);
                }
            };
            let config = Arc::new(config);
            init_logging(Arc::clone(&config))?;
            serve(config).await?;
        }

        Commands::ValidateConfig { config, display } => {
            let config = match FaberConfig::load_from_path(&config) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("Failed to load configuration from {}: {}", config, e);
                    exit(1);
                }
            };
            info!("Validating configuration...");
            if display {
                println!("{config:#?}");
            } else {
                println!("Config validated successfully");
            }
            info!("Configuration validation completed");
        }
    };

    info!("CLI run completed successfully");
    Ok(())
}
