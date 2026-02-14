use std::time::Duration;

#[derive(Debug, Clone)]
pub enum BackendConfig {
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

#[derive(Debug, Clone)]
pub struct StoreConfig {
    pub backend: BackendConfig,
    pub default_ttl: Duration,
    pub ttl_check_interval: Duration,
    pub max_file_size: u64,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            backend: BackendConfig::Memory,
            default_ttl: Duration::from_secs(3600),
            ttl_check_interval: Duration::from_secs(60),
            max_file_size: 50 * 1024 * 1024,
        }
    }
}

impl StoreConfig {
    pub fn from_env() -> Self {
        let backend = match std::env::var("FABER_STORE_BACKEND")
            .unwrap_or_default()
            .as_str()
        {
            "filesystem" => {
                let path = std::env::var("FABER_STORE_PATH")
                    .unwrap_or_else(|_| "/var/lib/faber/store".to_string());
                BackendConfig::Filesystem { path }
            }
            "hybrid" => {
                let path = std::env::var("FABER_STORE_PATH")
                    .unwrap_or_else(|_| "/var/lib/faber/store".to_string());
                let max_memory_entries = std::env::var("FABER_STORE_MAX_MEMORY_ENTRIES")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1000);
                let max_memory_size = std::env::var("FABER_STORE_MAX_MEMORY_SIZE")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100 * 1024 * 1024); // 100MB default
                BackendConfig::Hybrid {
                    path,
                    max_memory_entries,
                    max_memory_size,
                }
            }
            _ => BackendConfig::Memory,
        };

        let default_ttl = std::env::var("FABER_STORE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(3600));

        let ttl_check_interval = std::env::var("FABER_STORE_TTL_CHECK_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(60));

        let max_file_size = std::env::var("FABER_STORE_MAX_FILE_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50 * 1024 * 1024);

        Self {
            backend,
            default_ttl,
            ttl_check_interval,
            max_file_size,
        }
    }

    pub fn builder() -> StoreConfigBuilder {
        StoreConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct StoreConfigBuilder {
    backend: Option<BackendConfig>,
    default_ttl: Option<Duration>,
    ttl_check_interval: Option<Duration>,
    max_file_size: Option<u64>,
}

impl StoreConfigBuilder {
    pub fn backend(mut self, backend: BackendConfig) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn memory(mut self) -> Self {
        self.backend = Some(BackendConfig::Memory);
        self
    }

    pub fn filesystem(mut self, path: impl Into<String>) -> Self {
        self.backend = Some(BackendConfig::Filesystem { path: path.into() });
        self
    }

    pub fn hybrid(
        mut self,
        path: impl Into<String>,
        max_memory_entries: usize,
        max_memory_size: u64,
    ) -> Self {
        self.backend = Some(BackendConfig::Hybrid {
            path: path.into(),
            max_memory_entries,
            max_memory_size,
        });
        self
    }

    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = Some(ttl);
        self
    }

    pub fn ttl_check_interval(mut self, interval: Duration) -> Self {
        self.ttl_check_interval = Some(interval);
        self
    }

    pub fn max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = Some(size);
        self
    }

    pub fn build(self) -> StoreConfig {
        let defaults = StoreConfig::default();
        StoreConfig {
            backend: self.backend.unwrap_or(defaults.backend),
            default_ttl: self.default_ttl.unwrap_or(defaults.default_ttl),
            ttl_check_interval: self
                .ttl_check_interval
                .unwrap_or(defaults.ttl_check_interval),
            max_file_size: self.max_file_size.unwrap_or(defaults.max_file_size),
        }
    }
}
