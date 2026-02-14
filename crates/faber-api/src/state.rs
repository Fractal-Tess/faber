use crate::cache::ExecutionCache;
use faber_store::FileStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub cache: ExecutionCache,
    pub file_store: Arc<dyn FileStore>,
    pub api_key: String,
    pub cache_enabled: bool,
}

impl AppState {
    pub fn new(api_key: String, cache_enabled: bool, file_store: Arc<dyn FileStore>) -> Self {
        Self {
            cache: ExecutionCache::new(),
            file_store,
            api_key,
            cache_enabled,
        }
    }
}
