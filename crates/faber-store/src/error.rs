use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Failed to store file: {0}")]
    StorageError(String),

    #[error("Failed to read file: {0}")]
    ReadError(String),

    #[error("Failed to delete file: {0}")]
    DeleteError(String),

    #[error("Failed to serialize metadata: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("File too large: {0} bytes (max: {1})")]
    FileTooLarge(u64, u64),

    #[error("Invalid file ID: {0}")]
    InvalidFileId(String),
}

pub type StoreResult<T> = std::result::Result<T, StoreError>;
