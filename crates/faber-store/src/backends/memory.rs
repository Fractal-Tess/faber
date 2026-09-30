use crate::config::StoreConfig;
use crate::error::{StoreError, StoreResult};
use crate::quota::Usage;
use crate::store::FileStore;
use crate::types::{FileId, FileInfo, FileMetadata, StoredFile, UploadResult, compute_file_id};
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::sync::Mutex;
use tracing::{debug, warn};

#[derive(Debug)]
pub struct MemoryStore {
    files: DashMap<FileId, StoredFile>,
    /// Held while checking and changing the file set so the quota cannot be
    /// overshot by concurrent uploads. Lock before touching `files`.
    usage: Mutex<Usage>,
    config: StoreConfig,
}

impl MemoryStore {
    pub fn new(config: StoreConfig) -> Self {
        Self {
            files: DashMap::new(),
            usage: Mutex::new(Usage::default()),
            config,
        }
    }

    fn lock_usage(&self) -> std::sync::MutexGuard<'_, Usage> {
        self.usage
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn remove(&self, id: &FileId) -> bool {
        let mut usage = self.lock_usage();
        match self.files.remove(id) {
            Some((_, file)) => {
                usage.remove(file.metadata.size);
                true
            }
            None => false,
        }
    }

    /// Remove `id` if it has expired; returns whether it did.
    fn expire(&self, id: &FileId) -> bool {
        let expired = self
            .files
            .get(id)
            .is_some_and(|entry| entry.metadata.is_expired());
        expired && self.remove(id)
    }
}

#[async_trait]
impl FileStore for MemoryStore {
    async fn put(&self, content: Bytes, mut metadata: FileMetadata) -> StoreResult<UploadResult> {
        let size = content.len() as u64;

        if size > self.config.max_file_size {
            return Err(StoreError::FileTooLarge(size, self.config.max_file_size));
        }

        let file_id = compute_file_id(&content);
        metadata.size = size;
        metadata.apply_default_ttl(self.config.default_ttl);
        self.expire(&file_id);

        let mut usage = self.lock_usage();
        if let Some(mut entry) = self.files.get_mut(&file_id) {
            entry.metadata.touch();
            debug!("File already exists: {}", file_id);
            return Ok(UploadResult {
                file_id,
                size,
                already_exists: true,
            });
        }
        usage.admit(size, &self.config)?;

        let stored_file = StoredFile {
            id: file_id.clone(),
            metadata,
            content: content.to_vec(),
        };
        self.files.insert(file_id.clone(), stored_file);
        usage.add(size);

        debug!("Stored file: {} ({} bytes)", file_id, size);

        Ok(UploadResult {
            file_id,
            size,
            already_exists: false,
        })
    }

    async fn get(&self, id: &FileId) -> StoreResult<StoredFile> {
        self.expire(id);
        let mut entry = self
            .files
            .get_mut(id)
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;

        entry.metadata.touch();

        Ok(entry.clone())
    }

    async fn exists(&self, id: &FileId) -> StoreResult<bool> {
        self.expire(id);
        Ok(self.files.contains_key(id))
    }

    async fn get_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        self.expire(id);
        let entry = self
            .files
            .get(id)
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;

        Ok(entry.metadata.clone())
    }

    async fn delete(&self, id: &FileId) -> StoreResult<bool> {
        if self.remove(id) {
            debug!("Deleted file: {}", id);
            Ok(true)
        } else {
            warn!("Attempted to delete non-existent file: {}", id);
            Ok(false)
        }
    }

    async fn list(&self) -> StoreResult<Vec<FileInfo>> {
        let files: Vec<FileInfo> = self
            .files
            .iter()
            .filter(|entry| !entry.metadata.is_expired())
            .map(|entry| FileInfo::from(entry.value()))
            .collect();

        Ok(files)
    }

    async fn touch(&self, id: &FileId) -> StoreResult<()> {
        self.expire(id);
        let mut entry = self
            .files
            .get_mut(id)
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;

        entry.metadata.touch();

        Ok(())
    }

    async fn purge_expired(&self) -> StoreResult<usize> {
        let expired: Vec<FileId> = self
            .files
            .iter()
            .filter(|entry| entry.metadata.is_expired())
            .map(|entry| entry.key().clone())
            .collect();
        Ok(expired.iter().filter(|id| self.expire(id)).count())
    }
}
