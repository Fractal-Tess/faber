use faber_runtime::{ExecutionStepResult, TaskGroup, TaskGroupResult, TaskOutcome, TaskResult};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Bounds of the whole-request result cache.
#[derive(Clone, Copy, Debug)]
pub struct CacheLimits {
    pub max_entries: usize,
    /// Output bytes held across every cached result.
    pub max_bytes: usize,
    pub ttl: Duration,
}

impl Default for CacheLimits {
    fn default() -> Self {
        Self {
            max_entries: 1024,
            max_bytes: 64 * 1024 * 1024,
            ttl: Duration::from_secs(300),
        }
    }
}

struct Entry {
    result: TaskGroupResult,
    bytes: usize,
    stored: Instant,
}

#[derive(Default)]
struct Entries {
    by_hash: HashMap<String, Entry>,
    /// Hashes in insertion order; the front is evicted first.
    order: VecDeque<String>,
    bytes: usize,
}

impl Entries {
    fn remove(&mut self, hash: &str) {
        if let Some(entry) = self.by_hash.remove(hash) {
            self.bytes -= entry.bytes;
            self.order.retain(|queued| queued != hash);
        }
    }
}

/// Memoized results of identical requests, bounded by entry count, output
/// bytes and age. Off unless `CACHE_ENABLED` is set: replaying a result is
/// only correct for deterministic requests.
#[derive(Clone)]
pub struct ExecutionCache {
    entries: Arc<Mutex<Entries>>,
    limits: CacheLimits,
}

impl ExecutionCache {
    pub fn new(limits: CacheLimits) -> Self {
        Self {
            entries: Arc::new(Mutex::new(Entries::default())),
            limits,
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

    /// Only a request whose every task ran to a clean exit with complete
    /// output is worth replaying. Timeouts, kills, limit hits and setup
    /// failures depend on load and must be retried, not remembered.
    fn cacheable(result: &TaskGroupResult) -> bool {
        let clean = |task: &TaskResult| {
            matches!(
                task,
                TaskResult::Completed { exit_code: 0, stats, .. }
                    if stats.outcome == TaskOutcome::Exited
                        && !stats.stdout_truncated
                        && !stats.stderr_truncated
            )
        };
        result.iter().all(|step| match step {
            ExecutionStepResult::Single(task) => clean(task),
            ExecutionStepResult::Parallel(tasks) => tasks.iter().all(clean),
        })
    }

    fn output_bytes(result: &TaskGroupResult) -> usize {
        let task_bytes = |task: &TaskResult| match task {
            TaskResult::Completed { stdout, stderr, .. } => stdout.len() + stderr.len(),
            TaskResult::Failed { error, .. } => error.len(),
        };
        result
            .iter()
            .map(|step| match step {
                ExecutionStepResult::Single(task) => task_bytes(task),
                ExecutionStepResult::Parallel(tasks) => tasks.iter().map(task_bytes).sum(),
            })
            .sum()
    }

    /// Store `result` if it is cacheable and fits; returns whether it was.
    pub fn cache_result(&self, hash: String, result: TaskGroupResult) -> bool {
        let bytes = Self::output_bytes(&result);
        if !Self::cacheable(&result)
            || bytes > self.limits.max_bytes
            || self.limits.max_entries == 0
        {
            return false;
        }
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        entries.remove(&hash);
        while entries.by_hash.len() >= self.limits.max_entries
            || entries.bytes + bytes > self.limits.max_bytes
        {
            let Some(oldest) = entries.order.front().cloned() else {
                break;
            };
            entries.remove(&oldest);
        }
        entries.bytes += bytes;
        entries.order.push_back(hash.clone());
        entries.by_hash.insert(
            hash,
            Entry {
                result,
                bytes,
                stored: Instant::now(),
            },
        );
        true
    }

    pub fn try_from_hash(&self, hash: &str) -> Option<TaskGroupResult> {
        let mut entries = self.entries.lock().ok()?;
        let expired = entries.by_hash.get(hash)?.stored.elapsed() >= self.limits.ttl;
        if expired {
            entries.remove(hash);
            return None;
        }
        entries.by_hash.get(hash).map(|entry| entry.result.clone())
    }

    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .map_or(0, |entries| entries.by_hash.len())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ExecutionCache {
    fn default() -> Self {
        Self::new(CacheLimits::default())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use faber_runtime::{
        ExecutionStep, ExecutionStepResult, Task, TaskOutcome, TaskResult, TaskResultStats,
    };

    use super::{CacheLimits, ExecutionCache};

    fn result(stdout: &str, exit_code: i32, outcome: TaskOutcome) -> Vec<ExecutionStepResult> {
        vec![ExecutionStepResult::Single(TaskResult::Completed {
            stdout: stdout.to_string(),
            stderr: String::new(),
            exit_code,
            stats: TaskResultStats {
                outcome,
                ..TaskResultStats::default()
            },
        })]
    }

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

    #[test]
    fn only_clean_complete_results_are_cached() {
        let cache = ExecutionCache::default();
        assert!(cache.cache_result("ok".into(), result("out", 0, TaskOutcome::Exited)));
        assert!(!cache.cache_result("failed".into(), result("out", 1, TaskOutcome::Exited)));
        assert!(!cache.cache_result("slow".into(), result("", 137, TaskOutcome::TimedOut)));
        assert!(!cache.cache_result("oom".into(), result("", 137, TaskOutcome::OutOfMemory)));
        assert!(cache.try_from_hash("ok").is_some());
        assert!(cache.try_from_hash("failed").is_none());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn entries_bytes_and_age_are_bounded() {
        let cache = ExecutionCache::new(CacheLimits {
            max_entries: 2,
            max_bytes: 10,
            ttl: Duration::from_secs(60),
        });
        assert!(cache.cache_result("a".into(), result("1234", 0, TaskOutcome::Exited)));
        assert!(cache.cache_result("b".into(), result("1234", 0, TaskOutcome::Exited)));
        // A third entry evicts the oldest, by count and by bytes alike.
        assert!(cache.cache_result("c".into(), result("1234", 0, TaskOutcome::Exited)));
        assert!(cache.try_from_hash("a").is_none());
        assert!(cache.try_from_hash("b").is_some() && cache.try_from_hash("c").is_some());
        // Larger than the whole cache: refused, nothing evicted for it.
        assert!(!cache.cache_result("big".into(), result("12345678901", 0, TaskOutcome::Exited)));
        assert_eq!(cache.len(), 2);

        let expiring = ExecutionCache::new(CacheLimits {
            ttl: Duration::ZERO,
            ..CacheLimits::default()
        });
        assert!(expiring.cache_result("a".into(), result("x", 0, TaskOutcome::Exited)));
        assert!(expiring.try_from_hash("a").is_none());
        assert!(expiring.is_empty());
    }
}
