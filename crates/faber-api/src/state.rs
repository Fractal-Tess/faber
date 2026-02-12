use crate::cache::ExecutionCache;

#[derive(Clone)]
pub struct AppState {
    pub cache: ExecutionCache,
    pub api_key: String,
    pub cache_enabled: bool,
}

impl AppState {
    pub fn new(api_key: String, cache_enabled: bool) -> Self {
        Self {
            cache: ExecutionCache::new(),
            api_key,
            cache_enabled,
        }
    }
}
