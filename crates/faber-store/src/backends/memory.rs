use crate::config::StoreConfig;
use crate::error::{StoreError, StoreResult};
use crate::store::FileStore;
use crate::types::{FileId, FileInfo, FileMetadata, StoredFile, UploadResult, compute_file_id};
use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use tracing::{debug, warn};

#[derive(Debug)]
pub struct MemoryStore {
    files: DashMap<FileId, StoredFile>,
    config: StoreConfig,
}

impl MemoryStore {
    pub fn new(config: StoreConfig) -> Self {
        Self {
            files: DashMap::new(),
            config,
        }
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

        if let Some(entry) = self.files.get(&file_id) {
            metadata.ttl = metadata.ttl.or(entry.metadata.ttl);
            debug!("File already exists: {}", file_id);
            return Ok(UploadResult {
                file_id,
                size: metadata.size,
                already_exists: true,
            });
        }

        let stored_file = StoredFile {
            id: file_id.clone(),
            metadata,
            content: content.to_vec(),
        };

        self.files.insert(file_id.clone(), stored_file);

        debug!("Stored file: {} ({} bytes)", file_id, size);

        Ok(UploadResult {
            file_id,
            size,
            already_exists: false,
        })
    }

    async fn get(&self, id: &FileId) -> StoreResult<StoredFile> {
        let mut entry = self
            .files
            .get_mut(id)
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;

        entry.metadata.touch();

        Ok(entry.clone())
    }

    async fn exists(&self, id: &FileId) -> StoreResult<bool> {
        Ok(self.files.contains_key(id))
    }

    async fn get_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        let entry = self
            .files
            .get(id)
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;

        Ok(entry.metadata.clone())
    }

    async fn delete(&self, id: &FileId) -> StoreResult<bool> {
        match self.files.remove(id) {
            Some((_, _file)) => {
                debug!("Deleted file: {}", id);
                Ok(true)
            }
            None => {
                warn!("Attempted to delete non-existent file: {}", id);
                Ok(false)
            }
        }
    }

    async fn list(&self) -> StoreResult<Vec<FileInfo>> {
        let files: Vec<FileInfo> = self
            .files
            .iter()
            .map(|entry| FileInfo::from(entry.value()))
            .collect();

        Ok(files)
    }

    async fn touch(&self, id: &FileId) -> StoreResult<()> {
        let mut entry = self
            .files
            .get_mut(id)
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;

        entry.metadata.touch();

        Ok(())
    }
}
