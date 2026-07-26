use crate::config::StoreConfig;
use crate::error::{StoreError, StoreResult};
use crate::store::FileStore;
use crate::types::{FileId, FileInfo, FileMetadata, StoredFile, UploadResult, compute_file_id};
use async_trait::async_trait;
use bytes::Bytes;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tokio::fs;
use tracing::{debug, warn};

#[derive(Debug)]
pub struct FilesystemStore {
    base_path: PathBuf,
    config: StoreConfig,
}

impl FilesystemStore {
    pub fn new(path: String, config: StoreConfig) -> Self {
        Self {
            base_path: PathBuf::from(path),
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

        let file_path = self.get_file_path(&file_id);
        let metadata_path = self.get_metadata_path(&file_id);

        if file_path.exists() {
            debug!("File already exists: {}", file_id);
            return Ok(UploadResult {
                file_id,
                size: metadata.size,
                already_exists: true,
            });
        }

        self.ensure_prefix_dirs(&file_id).await?;

        let parent_dir = file_path.parent().ok_or_else(|| {
            StoreError::StorageError("No parent directory for file path".to_string())
        })?;
        let mut temp_file = NamedTempFile::new_in(parent_dir)?;
        temp_file.write_all(&content)?;
        temp_file.flush()?;
        temp_file.as_file().sync_all()?;
        temp_file.persist(&file_path).map_err(|e| e.error)?;

        let metadata_json = serde_json::to_string(&metadata)?;
        let meta_parent = metadata_path.parent().ok_or_else(|| {
            StoreError::StorageError("No parent directory for metadata path".to_string())
        })?;
        let mut meta_temp = NamedTempFile::new_in(meta_parent)?;
        meta_temp.write_all(metadata_json.as_bytes())?;
        meta_temp.flush()?;
        meta_temp.as_file().sync_all()?;
        meta_temp.persist(&metadata_path).map_err(|e| e.error)?;

        debug!("Stored file: {} ({} bytes)", file_id, size);

        Ok(UploadResult {
            file_id,
            size,
            already_exists: false,
        })
    }

    async fn get(&self, id: &FileId) -> StoreResult<StoredFile> {
        let file_path = self.get_file_path(id);
        let metadata_path = self.get_metadata_path(id);

        if !file_path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }

        let content = fs::read(&file_path).await?;

        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let mut metadata: FileMetadata = serde_json::from_str(&metadata_json)?;
        metadata.touch();

        let updated_meta = serde_json::to_string(&metadata)?;
        fs::write(&metadata_path, updated_meta).await?;

        Ok(StoredFile {
            id: id.clone(),
            metadata,
            content,
        })
    }

    async fn exists(&self, id: &FileId) -> StoreResult<bool> {
        let file_path = self.get_file_path(id);
        Ok(file_path.exists())
    }

    async fn get_metadata(&self, id: &FileId) -> StoreResult<FileMetadata> {
        let metadata_path = self.get_metadata_path(id);

        if !metadata_path.exists() {
            return Err(StoreError::NotFound(id.to_string()));
        }

        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let metadata: FileMetadata = serde_json::from_str(&metadata_json)?;
        Ok(metadata)
    }

    async fn delete(&self, id: &FileId) -> StoreResult<bool> {
        let file_path = self.get_file_path(id);
        let metadata_path = self.get_metadata_path(id);

        let file_existed = file_path.exists();

        if file_existed {
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
