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

    pub fn generate_hash(task_group: &TaskGroup) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(task_group)?;
        Self::canonicalize_json(&mut value);
        let serialized = serde_json::to_vec(&value)?;
        let mut hasher = Sha256::new();
        hasher.update(serialized);
        Ok(format!("{:x}", hasher.finalize()))
    }

    fn canonicalize_json(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                let old = std::mem::take(object);
                let mut entries: Vec<_> = old.into_iter().collect();
                entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
                for (key, mut value) in entries {
                    Self::canonicalize_json(&mut value);
                    object.insert(key, value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    Self::canonicalize_json(value);
                }
            }
            _ => {}
        }
    }

    pub fn cache_result(&self, hash: String, result: TaskGroupResult) {
        self.cache.insert(hash, result);
    }

    pub fn try_from_hash(&self, hash: &String) -> Option<TaskGroupResult> {
        self.cache.get(hash).map(|entry| entry.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use faber_runtime::{ExecutionStep, Task};

    use super::ExecutionCache;

    #[test]
    fn cache_hash_is_stable_across_map_insertion_order() {
        let first = Task {
            cmd: "env".to_string(),
            args: None,
            env: Some(HashMap::from([
                ("B".to_string(), "2".to_string()),
                ("A".to_string(), "1".to_string()),
            ])),
            stdin: None,
            files: None,
            working_dir: None,
            sandbox_profile: None,
        };
        let second = Task {
            env: Some(HashMap::from([
                ("A".to_string(), "1".to_string()),
                ("B".to_string(), "2".to_string()),
            ])),
            ..first.clone()
        };

        let first_hash = ExecutionCache::generate_hash(&vec![ExecutionStep::Single(first)]);
        let second_hash = ExecutionCache::generate_hash(&vec![ExecutionStep::Single(second)]);
        assert_eq!(first_hash.unwrap(), second_hash.unwrap());
    }
}

impl Default for ExecutionCache {
    fn default() -> Self {
        Self::new()
    }
}
