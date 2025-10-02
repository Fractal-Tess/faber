use crate::cache::ExecutionCache;

#[derive(Clone)]
pub struct AppState {
    pub cache: ExecutionCache,
    pub api_key: String,
}

impl AppState {
    pub fn new(api_key: String) -> Self {
        Self {
            cache: ExecutionCache::new(),
            api_key,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cache: ExecutionCache::new(),
            api_key: "default-api-key".to_string(), // This should never be used in production
        }
    }
}
