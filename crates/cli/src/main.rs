use clap::{Parser, Subcommand};
use faber_api::create_router;
use faber_config::Config;
use tracing::{Level, error, info};

#[derive(Parser)]
#[command(name = "faber")]
#[command(about = "A secure sandboxed task execution service")]
#[command(version)]
#[command(propagate_version = true)]
struct Cli {
    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    log_level: Option<Level>,

    /// Configuration file path
    #[arg(short, long, default_value = "config/config.yaml")]
    config: Option<String>,

    /// Enable debug mode
    #[arg(short, long)]
    debug: bool,

    /// Enable open mode (no authentication)
    #[arg(long)]
    open_mode: bool,

    /// Host to bind to
    #[arg(long)]
    host: Option<String>,

    /// Port to bind to
    #[arg(short, long)]
    port: Option<u16>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Faber server
    Serve {
        /// Enable graceful shutdown
        #[arg(long)]
        graceful_shutdown: bool,
    },
    /// Validate configuration
    Validate {
        /// Configuration file to validate (uses --config if not specified)
        config: Option<String>,
    },
    /// Show configuration
    Config {
        /// Show default configuration
        #[arg(long)]
        default: bool,
    },
    /// Execute a single task (for testing)
    Execute {
        /// Command to execute
        command: String,
        /// Command arguments
        args: Vec<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(cli.log_level.unwrap_or(Level::INFO), cli.debug);

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
        Some(Commands::Execute { command, args }) => {
            execute_task(command, args).await?;
        }
        None => {
            // Default to serve if no subcommand
            serve(cli, false).await?;
        }
    }
    Ok(())
}

async fn serve(cli: Cli, graceful_shutdown: bool) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Faber...");

    // Load configuration
    let config_path = cli.config.unwrap_or("config/config.yaml".to_owned());
    info!("Loading configuration from {}", config_path);
    let mut config = Config::from_file(config_path)?;

    // Override with CLI options
    if cli.open_mode {
        config.auth.open_mode = true;
    }
    if let Some(host) = cli.host {
        config.server.host = host;
    }
    if let Some(port) = cli.port {
        config.server.port = port;
    }

    info!("Configuration loaded successfully");
    info!("{}", config);

    let app = create_router(&config);

    let listener =
        tokio::net::TcpListener::bind(&format!("{}:{}", config.server.host, config.server.port))
            .await?;
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

fn validate_config(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
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

fn show_config(
    default: bool,
    config_path: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if default {
        let config = Config::default();
        println!("{}", serde_json::to_string_pretty(&config)?);
    } else {
        let path = config_path.as_deref().unwrap_or("config/config.yaml");
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

async fn execute_task(
    command: String,
    args: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Executing task: {} {:?}", command, args);

    // This would integrate with the executor crate
    // For now, just show what would be executed
    println!("Command: {command}");
    println!("Arguments: {args:?}");
    println!("(Task execution not yet implemented in CLI)");

    Ok(())
}

fn init_logging(level: Level, debug: bool) {
    let env_filter = if debug {
        "debug"
    } else {
        match level {
            Level::ERROR => "error",
            Level::WARN => "warn",
            Level::INFO => "info",
            Level::DEBUG => "debug",
            Level::TRACE => "trace",
        }
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .init();
}
