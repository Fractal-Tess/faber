#![warn(clippy::unwrap_used)]

mod api;
mod cli;
mod config;
mod logging;

use cli::run_cli;
use tracing::error;

#[tokio::main]
async fn main() {
    if let Err(e) = run_cli().await {
        error!("Application error: {}", e);
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
