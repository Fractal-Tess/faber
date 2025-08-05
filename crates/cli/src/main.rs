use clap::{CommandFactory, Parser};
use faber_config::FaberConfig;
use tracing::{Level, error};

use faber_cli::{Cli, Commands, init_logging, serve};

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

    match cli.command {
        Some(Commands::Serve {
            auth_enabled,
            host,
            port,
            workers,
            log_dir,
            log_level,
            config,
        }) => {
            todo!();
        }
        Some(Commands::ValidateConfig { display }) => {
            let config = FaberConfig::load_from_path(cli.config).expect("Failed to load config");
            if display {
                println!("{config:#?}");
            } else {
                println!("Config validated successfully");
            }
        }
        None => {
            Cli::command()
                .print_help()
                .map_err(|e| {
                    error!("Failed to print help: {e}");
                })
                .expect("Failed to print help");
            std::process::exit(0);
        }
    }
}
