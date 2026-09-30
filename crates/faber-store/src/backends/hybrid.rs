use crate::backends::FilesystemStore;
use crate::config::StoreConfig;
use crate::error::StoreResult;
use crate::lru::LruCache;
use crate::store::FileStore;
use crate::types::{FileId, FileInfo, FileMetadata, StoredFile, UploadResult};
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Mutex;
use tracing::debug;

/// Filesystem store with an LRU memory cache in front of it. The disk is
/// authoritative for content, quota and expiry; the cache only saves reads.
#[derive(Debug)]
pub struct HybridStore {
    disk: FilesystemStore,
    memory_cache: DashMap<FileId, StoredFile>,
    lru: Mutex<LruCache>,
    max_memory_size: u64,
}

impl HybridStore {
    pub fn new(
        path: String,
        max_memory_entries: usize,
        max_memory_size: u64,
        config: StoreConfig,
    ) -> Self {
        Self {
            disk: FilesystemStore::new(path, config),
            memory_cache: DashMap::new(),
            lru: Mutex::new(LruCache::new(max_memory_entries, max_memory_size)),
            max_memory_size,
        }
    }

    fn lock_lru(&self) -> std::sync::MutexGuard<'_, LruCache> {
        self.lru
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn promote_to_memory(&self, file: StoredFile) {
        let size = file.metadata.size;
        if size > self.max_memory_size {
            return;
        }
        let id = file.id.clone();

        let mut lru = self.lock_lru();
        for evict_id in lru.evict_until_space(size) {
            if let Ok(evict_id) = FileId::new(evict_id) {
                self.memory_cache.remove(&evict_id);
                debug!("Evicted from memory cache: {}", evict_id);
            }
        }
        lru.insert(id.to_string(), size);
        self.memory_cache.insert(id.clone(), file);
        debug!("Promoted to memory cache: {}", id);
    }

    fn evict_from_memory(&self, id: &FileId) {
        self.memory_cache.remove(id);
        self.lock_lru().remove(id.as_str());
    }
}

#[async_trait]
impl FileStore for HybridStore {
    async fn put(&self, content: Bytes, metadata: FileMetadata) -> StoreResult<UploadResult> {
        let result = self.disk.put(content.clone(), metadata).await?;
        if !self.memory_cache.contains_key(&result.file_id) {
            let metadata = self.disk.get_metadata(&result.file_id).await?;
            self.promote_to_memory(StoredFile {
                id: result.file_id.clone(),
                metadata,
                content: content.to_vec(),
            });
        }
        Ok(result)
    }

    async fn get(&self, id: &FileId) -> StoreResult<StoredFile> {
        // Touch the disk copy on every read so expiry sees the access; this
        // also fails with NotFound (and evicts below) once the file expired.
        let cached = self.memory_cache.get(id).map(|entry| entry.clone());
        if let Some(mut file) = cached {
            match self.disk.touch(id).await {
                Ok(()) => {
                    file.metadata.touch();
                    if let Some(mut entry) = self.memory_cache.get_mut(id) {
                        entry.metadata.touch();
                    }
                    self.lock_lru().get(id.as_str());
                    debug!("Cache hit: {}", id);
                    return Ok(file);
                }
                Err(error) => {
                    self.evict_from_memory(id);
                    return Err(error);
                }
            }
        }

        debug!("Cache miss, loading from disk: {}", id);
        let file = self.disk.get(id).await?;
        self.promote_to_memory(file.clone());
        Ok(file)
    }

    async fn exists(&self, id: &FileId) -> StoreResult<bool> {
        let exists = self.disk.exists(id).await?;
        if !exists {
            self.evict_from_memory(id);
        }
        Ok(exists)
    }

    async fn get_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        let metadata = self.disk.get_metadata(id).await;
        if metadata.is_err() {
            self.evict_from_memory(id);
        }
        metadata
    }

    async fn delete(&self, id: &FileId) -> StoreResult<bool> {
        self.evict_from_memory(id);
        self.disk.delete(id).await
    }

    async fn list(&self) -> StoreResult<Vec<FileInfo>> {
        self.disk.list().await
    }

    async fn touch(&self, id: &FileId) -> StoreResult<()> {
        match self.disk.touch(id).await {
            Ok(()) => {
                if let Some(mut entry) = self.memory_cache.get_mut(id) {
                    entry.metadata.touch();
                }
                self.lock_lru().get(id.as_str());
                Ok(())
            }
            Err(error) => {
                self.evict_from_memory(id);
                Err(error)
            }
        }
    }

    async fn purge_expired(&self) -> StoreResult<usize> {
        let removed = self.disk.purge_expired_ids().await?;
        for id in &removed {
            self.evict_from_memory(id);
        }
        Ok(removed.len())
    }
}
