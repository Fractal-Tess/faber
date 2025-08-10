#![warn(clippy::unwrap_used)]

mod api;
mod cli;
mod config;
mod crash;
mod logging;

use cli::run_cli;

#[tokio::main]
async fn main() {
    eprintln!("=== Main function started ===");

    // Install crash signal logging as early as possible
    eprintln!("Installing crash handlers");
    crash::install_crash_handlers();
    eprintln!("Crash handlers installed");

    eprintln!("Running CLI");
    match run_cli().await {
        Ok(_) => {
            eprintln!("CLI completed successfully");
            eprintln!("=== Main function completed successfully ===");
        }
        Err(e) => {
            eprintln!("=== CLI ERROR ===");
            eprintln!("Error: {e}");
            eprintln!("Exiting with code 1");
            std::process::exit(1);
        }
    }
}
