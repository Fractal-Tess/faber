use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "faber")]
#[command(about = "A secure containerized task execution service")]
#[command(version)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "/faber/config/default.toml")]
    pub config: String,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the Faber server
    Serve {},
    /// Validate configuration, optionally display the parsed config
    ValidateConfig {
        /// Display the parsed configuration after validation
        #[arg(short, long)]
        display: bool,
    },
}
