use std::env;

pub struct ApiConfig {
    pub port: u16,
    pub host: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        // Load .env file in development, ignore errors in production
        dotenvy::dotenv().ok();

        // Get configuration from environment variables with defaults
        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap(),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
        }
    }
}

impl ApiConfig {
    pub fn new() -> Self {
        Self::default()
    }
}
