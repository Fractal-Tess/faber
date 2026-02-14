use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub host: String,
    pub max_concurrency: usize,
    pub api_key: String,
    pub cache_enabled: bool,
    pub store_backend: StoreBackend,
}

#[derive(Debug, Clone)]
pub enum StoreBackend {
    Memory,
    Filesystem {
        path: String,
    },
    Hybrid {
        path: String,
        max_memory_entries: usize,
        max_memory_size: u64,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Config {
            port: Self::load_port()?,
            host: Self::load_host(),
            max_concurrency: Self::load_max_concurrency()?,
            api_key: Self::load_api_key()?,
            cache_enabled: Self::load_cache_enabled(),
            store_backend: Self::load_store_backend(),
        })
    }

    fn load_port() -> Result<u16, Box<dyn std::error::Error + Send + Sync>> {
        let port_str = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
        port_str.parse::<u16>().map_err(|e| e.into())
    }

    fn load_host() -> String {
        env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string())
    }

    fn load_max_concurrency() -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let concurrency_str = env::var("MAX_CONCURRENCY").unwrap_or_else(|_| "10".to_string());
        concurrency_str.parse::<usize>().map_err(|e| e.into())
    }

    fn load_api_key() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        env::var("API_KEY")
            .map_err(|_| "API_KEY environment variable is required but not set".into())
    }

    fn load_cache_enabled() -> bool {
        env::var("CACHE_ENABLED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(true)
    }

    fn load_store_backend() -> StoreBackend {
        match env::var("FABER_STORE_BACKEND").unwrap_or_default().as_str() {
            "filesystem" => {
                let path = env::var("FABER_STORE_PATH")
                    .unwrap_or_else(|_| "/var/lib/faber/store".to_string());
                StoreBackend::Filesystem { path }
            }
            "hybrid" => {
                let path = env::var("FABER_STORE_PATH")
                    .unwrap_or_else(|_| "/var/lib/faber/store".to_string());
                let max_memory_entries = env::var("FABER_STORE_MAX_MEMORY_ENTRIES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1000);
                let max_memory_size = env::var("FABER_STORE_MAX_MEMORY_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100 * 1024 * 1024);
                StoreBackend::Hybrid {
                    path,
                    max_memory_entries,
                    max_memory_size,
                }
            }
            _ => StoreBackend::Memory,
        }
    }
}
