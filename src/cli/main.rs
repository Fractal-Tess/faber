use std::sync::Arc;

use clap::{CommandFactory, Parser};
use faber_api::serve;
use faber_cli::{Cli, Commands, init_logging};
use faber_config::FaberConfig;
use std::error::Error;
use std::process::exit;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

    let config = match FaberConfig::load_from_path(&cli.config) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("{e}");
            exit(1);
        }
    };

    let config = Arc::new(config);

    match command {
        Commands::Serve {} => {
            init_logging(Arc::clone(&config))?;
            serve(config).await?;
        }

        Commands::ValidateConfig { display } => {
            if display {
                println!("{config:#?}");
            } else {
                println!("Config validated successfully");
            }
        }
    };

    Ok(())
}
