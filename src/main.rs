#![allow(unused)]
#![warn(clippy::unwrap_used)]

mod api;
mod cli;
mod config;
mod container;
mod executor;
mod logging;

use cli::run;

#[tokio::main]
async fn main() {
    run().await;
}
