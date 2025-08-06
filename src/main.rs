#![allow(unused)]
#![warn(clippy::unwrap_used)]
use std::error::Error;

mod api;
mod cli;
mod config;
mod logging;

use cli::run;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    run().await
}
