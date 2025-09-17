use crate::cache::ExecutionCache;

#[derive(Clone)]
pub struct AppState {
    pub cache: ExecutionCache,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            cache: ExecutionCache::new(),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
