use std::env;
use std::fmt::Display;

#[derive(Debug)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub api_key: String,
    pub enable_swagger: bool,
    pub open: bool,
}

impl Default for Config {
    fn default() -> Self {
        // Load .env file in development, ignore errors in production
        dotenvy::dotenv().ok();

        // Check if OPEN mode is enabled
        let open = env::var("OPEN")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        // Get API key - required only when not in OPEN mode
        let api_key = if open {
            "open-mode-no-auth".to_string() // Placeholder when OPEN=true
        } else {
            env::var("API_KEY").expect("API_KEY environment variable must be set when OPEN=false")
        };

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
            open,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Dynamically print all fields of Config using Debug trait
        writeln!(f, "{self:#?}")
    }
}
