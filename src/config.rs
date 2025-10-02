use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub max_concurrency: usize,
    pub api_key: String,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Config {
            port: Self::load_port()?,
            host: Self::load_host(),
            max_concurrency: Self::load_max_concurrency()?,
            api_key: Self::load_api_key()?,
        })
    }

    /// Load the PORT environment variable, defaulting to 3000
    fn load_port() -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
        let port_str = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        port_str.parse::<u16>().map_err(|e| e.into())
    }

    /// Load the HOST environment variable, defaulting to "0.0.0.0"
    fn load_host() -> String {
        env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string())
    }

    /// Load the MAX_CONCURRENCY environment variable, defaulting to 10
    fn load_max_concurrency() -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let concurrency_str = env::var("MAX_CONCURRENCY").unwrap_or_else(|_| "10".to_string());
        concurrency_str.parse::<usize>().map_err(|e| e.into())
    }

    /// Load the API_KEY environment variable, required for server to start
    fn load_api_key() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        env::var("API_KEY").map_err(|_| {
            "API_KEY environment variable is required but not set".into()
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 3000,
            host: "0.0.0.0".to_string(),
            max_concurrency: 10,
            api_key: "default-api-key".to_string(), // This should never be used in production
        }
    }
}
