use crate::config::StoreConfig;
use crate::error::{StoreError, StoreResult};
use crate::lru::LruCache;
use crate::store::FileStore;
use crate::types::{FileId, FileInfo, FileMetadata, StoredFile, UploadResult, compute_file_id};
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

#[derive(Debug)]
pub struct HybridStore {
    memory_cache: DashMap<FileId, StoredFile>,
    lru: Mutex<LruCache>,
    base_path: PathBuf,
    max_memory_entries: usize,
    max_memory_size: u64,
    config: StoreConfig,
}

impl HybridStore {
    pub fn new(
        path: String,
        max_memory_entries: usize,
        max_memory_size: u64,
        config: StoreConfig,
    ) -> Self {
        Self {
            memory_cache: DashMap::new(),
            lru: Mutex::new(LruCache::new(max_memory_entries, max_memory_size)),
            base_path: PathBuf::from(path),
            max_memory_entries,
            max_memory_size,
            config,
        }
    }

    fn get_file_path(&self, id: &FileId) -> PathBuf {
        let hash = id.as_str();
        let prefix = &hash[..4];
        self.base_path.join("files").join(prefix).join(hash)
    }

    fn get_metadata_path(&self, id: &FileId) -> PathBuf {
        let hash = id.as_str();
        let prefix = &hash[..4];
        self.base_path
            .join("metadata")
            .join(prefix)
            .join(format!("{}.json", hash))
    }

    async fn ensure_prefix_dirs(&self, id: &FileId) -> StoreResult<()> {
        let hash = id.as_str();
        let prefix = &hash[..4];
        fs::create_dir_all(self.base_path.join("files").join(prefix)).await?;
        fs::create_dir_all(self.base_path.join("metadata").join(prefix)).await?;
        Ok(())
    }

    async fn store_to_disk(
        &self,
        id: &FileId,
        content: &[u8],
        metadata: &FileMetadata,
    ) -> StoreResult<()> {
        self.ensure_prefix_dirs(id).await?;

        let file_path = self.get_file_path(id);
        let metadata_path = self.get_metadata_path(id);

        let temp_path = file_path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);
        fs::rename(&temp_path, &file_path).await?;

        let metadata_json = serde_json::to_string(metadata)?;
        let meta_temp = metadata_path.with_extension("tmp");
        let mut meta_file = fs::File::create(&meta_temp).await?;
        meta_file.write_all(metadata_json.as_bytes()).await?;
        meta_file.sync_all().await?;
        drop(meta_file);
        fs::rename(&meta_temp, &metadata_path).await?;

        Ok(())
    }

    async fn load_from_disk(&self, id: &FileId) -> StoreResult<StoredFile> {
        let file_path = self.get_file_path(id);
        let metadata_path = self.get_metadata_path(id);

        if !file_path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }

        let content = fs::read(&file_path).await?;
        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let metadata: FileMetadata = serde_json::from_str(&metadata_json)?;

        Ok(StoredFile {
            id: id.clone(),
            metadata,
            content,
        })
    }

    fn promote_to_memory(&self, file: StoredFile) {
        let size = file.metadata.size;
        let id = file.id.clone();

        let mut lru = self.lru.lock().unwrap();
        let evicted = lru.evict_until_space(size);

        for evict_id in evicted {
            self.memory_cache.remove(&FileId::from(evict_id.clone()));
            debug!("Evicted from memory cache: {}", evict_id);
        }

        lru.insert(id.to_string(), size);
        self.memory_cache.insert(id.clone(), file);
        debug!("Promoted to memory cache: {}", id);
    }

    fn evict_from_memory(&self, id: &FileId) {
        self.memory_cache.remove(id);
        self.lru.lock().unwrap().remove(id.as_str());
    }
}

#[async_trait]
impl FileStore for HybridStore {
    async fn put(&self, content: Bytes, mut metadata: FileMetadata) -> StoreResult<UploadResult> {
        let size = content.len() as u64;

        if size > self.config.max_file_size {
            return Err(StoreError::FileTooLarge(size, self.config.max_file_size));
        }

        let file_id = compute_file_id(&content);
        metadata.size = size;

        // Always store to disk first
        self.store_to_disk(&file_id, &content, &metadata).await?;

        // Check if already in memory
        if self.memory_cache.contains_key(&file_id) {
            debug!("File already in memory cache: {}", file_id);
            return Ok(UploadResult {
                file_id,
                size,
                already_exists: true,
            });
        }

        // Check if file exists on disk (already stored)
        let file_path = self.get_file_path(&file_id);
        if file_path.exists() && !self.memory_cache.contains_key(&file_id) {
            // File exists on disk but not in memory - check if we should cache it
            let mut lru = self.lru.lock().unwrap();

            // If small enough and fits in cache, add to memory
            if size <= self.max_memory_size {
                let evicted = lru.evict_until_space(size);
                for evict_id in evicted {
                    self.memory_cache.remove(&FileId::from(evict_id));
                }
                lru.insert(file_id.to_string(), size);
                drop(lru);

                let stored_file = StoredFile {
                    id: file_id.clone(),
                    metadata: metadata.clone(),
                    content: content.to_vec(),
                };
                self.memory_cache.insert(file_id.clone(), stored_file);
                debug!("Cached in memory: {}", file_id);
            }

            return Ok(UploadResult {
                file_id,
                size,
                already_exists: true,
            });
        }

        // New file - try to cache in memory if it fits
        {
            let mut lru = self.lru.lock().unwrap();
            if size <= self.max_memory_size {
                let evicted = lru.evict_until_space(size);
                for evict_id in evicted {
                    self.memory_cache.remove(&FileId::from(evict_id));
                }
                lru.insert(file_id.to_string(), size);
            }
        }

        let stored_file = StoredFile {
            id: file_id.clone(),
            metadata,
            content: content.to_vec(),
        };
        self.memory_cache.insert(file_id.clone(), stored_file);

        debug!("Stored file (hybrid): {} ({} bytes)", file_id, size);

        Ok(UploadResult {
            file_id,
            size,
            already_exists: false,
        })
    }

    async fn get(&self, id: &FileId) -> StoreResult<StoredFile> {
        // Try memory first
        if let Some(mut entry) = self.memory_cache.get_mut(id) {
            entry.metadata.touch();
            self.lru.lock().unwrap().get(id.as_str());
            debug!("Cache hit: {}", id);
            return Ok(entry.clone());
        }

        // Fall back to disk
        debug!("Cache miss, loading from disk: {}", id);
        let file = self.load_from_disk(id).await?;

        // Promote to memory cache
        let size = file.metadata.size;
        if size <= self.max_memory_size {
            self.promote_to_memory(file.clone());
        }

        Ok(file)
    }

    async fn exists(&self, id: &FileId) -> StoreResult<bool> {
        if self.memory_cache.contains_key(id) {
            return Ok(true);
        }
        Ok(self.get_file_path(id).exists())
    }

    async fn get_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        // Try memory first
        if let Some(entry) = self.memory_cache.get(id) {
            return Ok(entry.metadata.clone());
        }

        // Fall back to disk
        let metadata_path = self.get_metadata_path(id);
        if !metadata_path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }

        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let metadata: FileMetadata = serde_json::from_str(&metadata_json)?;
        Ok(metadata)
    }

    async fn delete(&self, id: &FileId) -> StoreResult<bool> {
        // Remove from memory
        self.evict_from_memory(id);

        // Remove from disk
        let file_path = self.get_file_path(id);
        let metadata_path = self.get_metadata_path(id);

        let existed = file_path.exists();

        if existed {
            fs::remove_file(&file_path).await?;
            if metadata_path.exists() {
                fs::remove_file(&metadata_path).await?;
            }
            debug!("Deleted file: {}", id);
            Ok(true)
        } else {
            warn!("Attempted to delete non-existent file: {}", id);
            Ok(false)
        }
    }

    async fn list(&self) -> StoreResult<Vec<FileInfo>> {
        let files_path = self.base_path.join("files");

        if !files_path.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut prefix_dirs = fs::read_dir(&files_path).await?;

        while let Some(prefix_entry) = prefix_dirs.next_entry().await? {
            if prefix_entry.file_type().await?.is_dir() {
                let mut file_entries = fs::read_dir(prefix_entry.path()).await?;
                while let Some(file_entry) = file_entries.next_entry().await? {
                    if file_entry.file_type().await?.is_file() {
                        if let Some(name) = file_entry.file_name().to_str() {
                            let file_id = FileId::from(name);
                            if let Ok(metadata) = self.get_metadata(&file_id).await {
                                results.push(FileInfo {
                                    id: file_id,
                                    filename: metadata.filename,
                                    content_type: metadata.content_type,
                                    size: metadata.size,
                                    created_at: metadata.created_at,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    async fn touch(&self, id: &FileId) -> StoreResult<()> {
        // Update memory cache if present
        if let Some(mut entry) = self.memory_cache.get_mut(id) {
            entry.metadata.touch();
            self.lru.lock().unwrap().get(id.as_str());
        }

        // Update disk metadata
        let metadata_path = self.get_metadata_path(id);
        if !metadata_path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }

        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let mut metadata: FileMetadata = serde_json::from_str(&metadata_json)?;
        metadata.touch();

        let updated_meta = serde_json::to_string(&metadata)?;
        fs::write(&metadata_path, updated_meta).await?;

        Ok(())
    }
}
