use dashmap::DashMap;
use faber_runtime::{TaskGroup, TaskGroupResult};
use sha2::{Digest, Sha256};
use std::sync::Arc;

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

    fn generate_hash(task_group: &TaskGroup) -> String {
        let serialized = serde_json::to_string(task_group).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn cache_result(&self, task_group: TaskGroup, result: TaskGroupResult) {
        let hash = Self::generate_hash(&task_group);
        self.cache.insert(hash, result);
    }

    pub fn try_get(&self, task_group: &TaskGroup) -> Option<TaskGroupResult> {
        let hash = Self::generate_hash(task_group);
        self.cache.get(&hash).map(|entry| entry.clone())
    }
}

impl Default for ExecutionCache {
    fn default() -> Self {
        Self::new()
    }
}
