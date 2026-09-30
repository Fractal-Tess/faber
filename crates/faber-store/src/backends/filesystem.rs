use crate::config::StoreConfig;
use crate::error::{StoreError, StoreResult};
use crate::quota::Usage;
use crate::store::FileStore;
use crate::types::{FileId, FileInfo, FileMetadata, StoredFile, UploadResult, compute_file_id};
use async_trait::async_trait;
use bytes::Bytes;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::fs;
use tokio::sync::{Mutex, MutexGuard};
use tracing::{debug, warn};

#[derive(Debug)]
pub struct FilesystemStore {
    base_path: PathBuf,
    config: StoreConfig,
    /// Content usage, scanned from disk on first use. Held while checking and
    /// changing the file set so the quota cannot be overshot concurrently.
    usage: Mutex<Option<Usage>>,
}

impl FilesystemStore {
    pub fn new(path: String, config: StoreConfig) -> Self {
        Self {
            base_path: PathBuf::from(path),
            config,
            usage: Mutex::new(None),
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

    /// Replace `path` with `contents` through a temporary file and rename, so
    /// concurrent readers see either the old or the new file, never a torn one.
    fn write_atomically(path: &Path, contents: &[u8]) -> StoreResult<()> {
        let parent = path.parent().ok_or_else(|| {
            StoreError::StorageError(format!("No parent directory for {}", path.display()))
        })?;
        let mut temp = NamedTempFile::new_in(parent)?;
        temp.write_all(contents)?;
        temp.flush()?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
        Ok(())
    }

    async fn lock_usage(&self) -> StoreResult<MutexGuard<'_, Option<Usage>>> {
        let mut usage = self.usage.lock().await;
        if usage.is_none() {
            *usage = Some(self.scan_usage().await?);
        }
        Ok(usage)
    }

    async fn scan_usage(&self) -> StoreResult<Usage> {
        let mut usage = Usage::default();
        for id in self.stored_ids().await? {
            if let Ok(metadata) = fs::metadata(self.get_file_path(&id)).await {
                usage.add(metadata.len());
            }
        }
        Ok(usage)
    }

    async fn stored_ids(&self) -> StoreResult<Vec<FileId>> {
        let files_path = self.base_path.join("files");
        if !files_path.exists() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        let mut prefix_dirs = fs::read_dir(&files_path).await?;
        while let Some(prefix_entry) = prefix_dirs.next_entry().await? {
            if !prefix_entry.file_type().await?.is_dir() {
                continue;
            }
            let mut file_entries = fs::read_dir(prefix_entry.path()).await?;
            while let Some(file_entry) = file_entries.next_entry().await? {
                if file_entry.file_type().await?.is_file()
                    && let Some(id) = file_entry
                        .file_name()
                        .to_str()
                        .and_then(|name| FileId::new(name).ok())
                {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    async fn read_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        let metadata_path = self.get_metadata_path(id);
        let metadata_json = match fs::read_to_string(&metadata_path).await {
            Ok(json) => json,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(StoreError::NotFound(id.to_string()));
            }
            Err(error) => return Err(error.into()),
        };
        Ok(serde_json::from_str(&metadata_json)?)
    }

    /// Remove a stored file and its metadata. Caller holds the usage lock.
    async fn remove_locked(&self, usage: &mut Usage, id: &FileId) -> StoreResult<bool> {
        let file_path = self.get_file_path(id);
        let size = match fs::metadata(&file_path).await {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        fs::remove_file(&file_path).await?;
        match fs::remove_file(self.get_metadata_path(id)).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        usage.remove(size);
        Ok(true)
    }

    /// Metadata for a live file, removing the file if it has expired.
    async fn live_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        let metadata = self.read_metadata(id).await?;
        if metadata.is_expired() {
            let mut usage = self.lock_usage().await?;
            let usage = usage.get_or_insert_with(Usage::default);
            // Re-check under the lock: a concurrent read may have renewed it.
            if self.read_metadata(id).await.is_ok_and(|m| m.is_expired()) {
                self.remove_locked(usage, id).await?;
            }
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(metadata)
    }

    async fn touch_metadata(&self, id: &FileId, metadata: &mut FileMetadata) -> StoreResult<()> {
        metadata.touch();
        let json = serde_json::to_vec(&metadata)?;
        Self::write_atomically(&self.get_metadata_path(id), &json)
    }

    /// Remove every expired file and return the removed identifiers.
    pub(crate) async fn purge_expired_ids(&self) -> StoreResult<Vec<FileId>> {
        let mut removed = Vec::new();
        for id in self.stored_ids().await? {
            let expired = match self.read_metadata(&id).await {
                Ok(metadata) => metadata.is_expired(),
                Err(_) => continue,
            };
            if !expired {
                continue;
            }
            let mut usage = self.lock_usage().await?;
            let usage = usage.get_or_insert_with(Usage::default);
            if self.read_metadata(&id).await.is_ok_and(|m| m.is_expired())
                && self.remove_locked(usage, &id).await?
            {
                removed.push(id);
            }
        }
        Ok(removed)
    }
}

#[async_trait]
impl FileStore for FilesystemStore {
    async fn put(&self, content: Bytes, mut metadata: FileMetadata) -> StoreResult<UploadResult> {
        let size = content.len() as u64;

        if size > self.config.max_file_size {
            return Err(StoreError::FileTooLarge(size, self.config.max_file_size));
        }

        let file_id = compute_file_id(&content);
        metadata.size = size;
        metadata.apply_default_ttl(self.config.default_ttl);

        let mut usage = self.lock_usage().await?;
        let usage = usage.get_or_insert_with(Usage::default);

        let file_path = self.get_file_path(&file_id);
        if file_path.exists() {
            match self.read_metadata(&file_id).await {
                Ok(mut existing) if !existing.is_expired() => {
                    self.touch_metadata(&file_id, &mut existing).await?;
                    debug!("File already exists: {}", file_id);
                    return Ok(UploadResult {
                        file_id,
                        size,
                        already_exists: true,
                    });
                }
                _ => {
                    self.remove_locked(usage, &file_id).await?;
                }
            }
        }
        usage.admit(size, &self.config)?;

        self.ensure_prefix_dirs(&file_id).await?;
        Self::write_atomically(&file_path, &content)?;
        Self::write_atomically(
            &self.get_metadata_path(&file_id),
            &serde_json::to_vec(&metadata)?,
        )?;
        usage.add(size);

        debug!("Stored file: {} ({} bytes)", file_id, size);

        Ok(UploadResult {
            file_id,
            size,
            already_exists: false,
        })
    }

    async fn get(&self, id: &FileId) -> StoreResult<StoredFile> {
        let mut metadata = self.live_metadata(id).await?;
        let content = match fs::read(self.get_file_path(id)).await {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(StoreError::NotFound(id.to_string()));
            }
            Err(error) => return Err(error.into()),
        };
        self.touch_metadata(id, &mut metadata).await?;

        Ok(StoredFile {
            id: id.clone(),
            metadata,
            content,
        })
    }

    async fn exists(&self, id: &FileId) -> StoreResult<bool> {
        match self.live_metadata(id).await {
            Ok(_) => Ok(self.get_file_path(id).exists()),
            Err(StoreError::NotFound(_)) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn get_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        self.live_metadata(id).await
    }

    async fn delete(&self, id: &FileId) -> StoreResult<bool> {
        let mut usage = self.lock_usage().await?;
        let usage = usage.get_or_insert_with(Usage::default);
        if self.remove_locked(usage, id).await? {
            debug!("Deleted file: {}", id);
            Ok(true)
        } else {
            warn!("Attempted to delete non-existent file: {}", id);
            Ok(false)
        }
    }

    async fn list(&self) -> StoreResult<Vec<FileInfo>> {
        let mut results = Vec::new();
        for id in self.stored_ids().await? {
            if let Ok(metadata) = self.read_metadata(&id).await
                && !metadata.is_expired()
            {
                results.push(FileInfo {
                    id,
                    filename: metadata.filename,
                    content_type: metadata.content_type,
                    size: metadata.size,
                    created_at: metadata.created_at,
                });
            }
        }
        Ok(results)
    }

    async fn touch(&self, id: &FileId) -> StoreResult<()> {
        let mut metadata = self.live_metadata(id).await?;
        self.touch_metadata(id, &mut metadata).await
    }

    async fn purge_expired(&self) -> StoreResult<usize> {
        Ok(self.purge_expired_ids().await?.len())
    }
}
