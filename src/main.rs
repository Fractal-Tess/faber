#![warn(clippy::unwrap_used)]

mod api;
mod cli;
mod config;
mod logging;
mod prelude;

use cli::run_cli;

#[tokio::main]
async fn main() {
    if let Err(e) = run_cli().await {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
