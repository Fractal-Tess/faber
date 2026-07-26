use crate::error::StoreResult;
use crate::types::{FileId, FileInfo, FileMetadata, StoredFile, UploadResult};
use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait FileStore: Send + Sync {
    async fn put(&self, content: Bytes, metadata: FileMetadata) -> StoreResult<UploadResult>;

    async fn get(&self, id: &FileId) -> StoreResult<StoredFile>;

    async fn exists(&self, id: &FileId) -> StoreResult<bool>;

    async fn get_metadata(&self, id: &FileId) -> StoreResult<FileMetadata>;

    async fn delete(&self, id: &FileId) -> StoreResult<bool>;

    async fn list(&self) -> StoreResult<Vec<FileInfo>>;

    async fn touch(&self, id: &FileId) -> StoreResult<()>;
}
