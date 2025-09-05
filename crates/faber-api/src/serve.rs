use axum::Router;
use tokio::net::TcpListener;

pub struct ServeConfig {
    pub port: u16,
    pub host: String,
    pub router: Router,
    pub start_message: Option<String>,
}

impl ServeConfig {
    pub fn new(
        port: u16,
        host: impl Into<String>,
        router: Router,
        start_message: Option<String>,
    ) -> Self {
        Self {
            port,
            host: host.into(),
            router,
            start_message,
        }
    }
}

pub async fn serve(config: ServeConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host, config.port)).await?;

    let message = config.start_message.unwrap_or_else(|| {
        format!(
            "Faber API server listening on {}:{}",
            config.host, config.port
        )
    });
    println!("{}", message);
    axum::serve(listener, config.router).await?;
    Ok(())
}
