use dashmap::DashMap;
use faber_runtime::{TaskGroup, TaskGroupResult};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Simple in-memory cache for execution results
#[derive(Clone)]
pub struct ExecutionCache {
    cache: Arc<DashMap<String, TaskGroupResult>>,
}

impl ExecutionCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(DashMap::new()),
        }
    }

    pub fn generate_hash(task_group: &TaskGroup) -> String {
        let serialized = serde_json::to_string(task_group).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn get(&self, hash: &str) -> Option<TaskGroupResult> {
        self.cache.get(hash).map(|entry| entry.clone())
    }

    pub fn insert(&self, hash: String, result: TaskGroupResult) {
        self.cache.insert(hash, result);
    }
}

impl Default for ExecutionCache {
    fn default() -> Self {
        Self::new()
    }
}
