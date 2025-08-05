use clap::{Parser, Subcommand};
use tracing::Level;

#[derive(Parser)]
#[command(name = "faber")]
#[command(about = "A secure sandboxed task execution service")]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the Faber server
    Serve {
        /// Enable auth
        #[arg(long)]
        auth_enabled: bool,

        /// Host to bind to
        #[arg(long)]
        host: Option<String>,

        /// Port to bind to
        #[arg(short, long)]
        port: Option<u16>,

        /// Number of worker threads for the execution queue
        #[arg(long)]
        workers: Option<usize>,

        /// Log directory path (if not specified, uses logs/)
        #[arg(long)]
        log_dir: Option<String>,

        /// Log level (error, warn, info, debug, trace)
        #[arg(short, long, default_value = Level::INFO.as_str())]
        log_level: Option<Level>,

        /// Configuration file path
        #[arg(short, long, default_value = "/faber/config/default.toml")]
        config: Option<String>,
    },
    /// Validate configuration
    ValidateConfig {
        /// Configuration file path
        #[arg(short, long, default_value = "/faber/confiog/default.toml")]
        config: Option<String>,
    },
}
