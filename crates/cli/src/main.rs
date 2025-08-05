use clap::{CommandFactory, Parser};
use faber_cli::{Cli, Commands, ServeOptions, init_logging, serve};
use faber_config::FaberConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            init_logging(&log_level, &log_dir)?;
            let options = ServeOptions::new(
                auth_enabled,
                host,
                port,
                workers,
                log_dir,
                log_level,
                config,
            );
            serve(options)?;
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
                    eprintln!("Failed to print help: {e}");
                })
                .expect("Failed to print help");
            std::process::exit(0);
        }
    };

    Ok(())
}
