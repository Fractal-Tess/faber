use clap::{Parser, Subcommand};
use tracing::Level;

#[derive(Parser)]
#[command(name = "faber")]
#[command(about = "A secure sandboxed task execution service")]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Log level (error, warn, info, debug, trace)
    #[arg(short, long, default_value = "info")]
    pub log_level: Option<Level>,

    /// Configuration file path
    #[arg(short, long, default_value = "config/default.toml")]
    pub config: Option<String>,

    /// Enable debug mode
    #[arg(short, long)]
    pub debug: bool,

    /// Enable open mode (no authentication)
    #[arg(long)]
    pub open_mode: bool,

    /// Host to bind to
    #[arg(long)]
    pub host: Option<String>,

    /// Port to bind to
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Log file path (if not specified, logs only to console)
    #[arg(long)]
    pub log_file: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
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
}
