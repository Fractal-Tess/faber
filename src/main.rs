#![allow(unused)]
#![warn(clippy::unwrap_used)]

mod api;
mod cli;
mod config;
mod logging;
mod sandbox;

use cli::run;

#[tokio::main]
async fn main() {
    run().await;
}
