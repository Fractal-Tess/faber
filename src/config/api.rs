use std::env;

pub struct ApiConfig {
    pub port: u16,
    pub host: String,
    pub api_key: String,
    pub enable_swagger: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        // Load .env file in development, ignore errors in production
        dotenvy::dotenv().ok();

        // Get configuration from environment variables with defaults
        let api_key = env::var("API_KEY").expect("API_KEY environment variable must be set");

        Self {
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap(),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            api_key,
            enable_swagger: env::var("ENABLE_SWAGGER")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        }
    }
}

impl ApiConfig {
    pub fn new() -> Self {
        Self::default()
    }
}
