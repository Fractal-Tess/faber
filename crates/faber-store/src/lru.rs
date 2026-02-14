use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct LruEntry {
    pub last_accessed: Instant,
    pub size: u64,
}

#[derive(Debug)]
pub struct LruCache {
    entries: HashMap<String, LruEntry>,
    access_order: Vec<String>,
    max_entries: usize,
    max_size: u64,
    current_size: u64,
}

impl LruCache {
    pub fn new(max_entries: usize, max_size: u64) -> Self {
        Self {
            entries: HashMap::new(),
            access_order: Vec::new(),
            max_entries,
            max_size,
            current_size: 0,
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.contains_key(id)
    }

    pub fn get(&mut self, id: &str) -> Option<&LruEntry> {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.last_accessed = Instant::now();
            self.access_order.retain(|k| k != id);
            self.access_order.push(id.to_string());
            return Some(entry);
        }
        None
    }

    pub fn insert(&mut self, id: String, size: u64) {
        if self.entries.contains_key(&id) {
            return;
        }

        self.entries.insert(
            id.clone(),
            LruEntry {
                last_accessed: Instant::now(),
                size,
            },
        );
        self.access_order.push(id.clone());
        self.current_size += size;
    }

    pub fn remove(&mut self, id: &str) -> Option<u64> {
        if let Some(entry) = self.entries.remove(id) {
            self.access_order.retain(|k| k != id);
            self.current_size -= entry.size;
            return Some(entry.size);
        }
        None
    }

    pub fn needs_eviction(&self) -> bool {
        self.entries.len() >= self.max_entries || self.current_size >= self.max_size
    }

    pub fn evict_lru(&mut self) -> Option<String> {
        if self.access_order.is_empty() {
            return None;
        }

        let lru_id = self.access_order.first().cloned();
        if let Some(id) = &lru_id {
            self.remove(id);
        }
        lru_id
    }

    pub fn evict_until_space(&mut self, needed_size: u64) -> Vec<String> {
        let mut evicted = Vec::new();

        while (self.entries.len() >= self.max_entries
            || self.current_size + needed_size > self.max_size)
            && !self.access_order.is_empty()
        {
            if let Some(id) = self.evict_lru() {
                evicted.push(id);
            } else {
                break;
            }
        }

        evicted
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn size(&self) -> u64 {
        self.current_size
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
        self.current_size = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_contains() {
        let mut cache = LruCache::new(3, 1000);
        cache.insert("a".to_string(), 100);
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn test_eviction() {
        let mut cache = LruCache::new(3, 1000);
        cache.insert("a".to_string(), 100);
        cache.insert("b".to_string(), 100);
        cache.insert("c".to_string(), 100);

        assert!(cache.needs_eviction());

        let evicted = cache.evict_lru();
        assert_eq!(evicted, Some("a".to_string()));
        assert!(!cache.contains("a"));
    }

    #[test]
    fn test_access_updates_order() {
        let mut cache = LruCache::new(3, 1000);
        cache.insert("a".to_string(), 100);
        cache.insert("b".to_string(), 100);
        cache.insert("c".to_string(), 100);

        cache.get("a");

        let evicted = cache.evict_lru();
        assert_eq!(evicted, Some("b".to_string()));
        assert!(cache.contains("a"));
    }

    #[test]
    fn test_size_tracking() {
        let mut cache = LruCache::new(10, 500);
        cache.insert("a".to_string(), 200);
        cache.insert("b".to_string(), 200);

        assert_eq!(cache.size(), 400);

        cache.remove("a");
        assert_eq!(cache.size(), 200);
    }

    #[test]
    fn test_evict_until_space() {
        let mut cache = LruCache::new(10, 300);
        cache.insert("a".to_string(), 100);
        cache.insert("b".to_string(), 100);
        cache.insert("c".to_string(), 100);

        let evicted = cache.evict_until_space(150);
        assert!(evicted.len() >= 2);
        assert!(cache.size() < 150);
    }
}
