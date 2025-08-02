use axum::{Router, routing::get};
use faber::api::health_check;

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    println!("Starting Faber...");
    let app = Router::new().route("/health", get(health_check));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let shutdown_signal = async {
        tokio::signal::ctrl_c().await.ok();
    };

    println!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .unwrap();

    println!("\nShutting down...");
}
