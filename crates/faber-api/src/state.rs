use crate::cache::ExecutionCache;
use faber_store::FileStore;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ExecutionLimits {
    pub memory_max: String,
    pub pids_max: u32,
    pub cpu_max: String,
    pub wall_timeout: Duration,
    pub cpu_time_limit: Duration,
    pub output_limit: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            memory_max: "256M".to_string(),
            pids_max: 64,
            cpu_max: "50000 100000".to_string(),
            wall_timeout: Duration::from_secs(5),
            cpu_time_limit: Duration::from_secs(5),
            output_limit: 1024 * 1024,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub cache: ExecutionCache,
    pub file_store: Arc<dyn FileStore>,
    pub api_key: String,
    pub cache_enabled: bool,
    pub execution_limits: ExecutionLimits,
}

impl AppState {
    pub fn new(
        api_key: String,
        cache_enabled: bool,
        file_store: Arc<dyn FileStore>,
        execution_limits: ExecutionLimits,
    ) -> Self {
        Self {
            cache: ExecutionCache::new(),
            file_store,
            api_key,
            cache_enabled,
            execution_limits,
        }
    }
}
