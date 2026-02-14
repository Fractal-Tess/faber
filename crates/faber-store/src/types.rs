use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId(pub String);

impl FileId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for FileId {
    fn from(s: String) -> Self {
        FileId(s)
    }
}

impl From<&str> for FileId {
    fn from(s: &str) -> Self {
        FileId(s.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: u64,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub ttl: Option<Duration>,
}

impl FileMetadata {
    pub fn new(size: u64) -> Self {
        let now = SystemTime::now();
        Self {
            filename: None,
            content_type: None,
            size,
            created_at: now,
            last_accessed: now,
            ttl: None,
        }
    }

    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn touch(&mut self) {
        self.last_accessed = SystemTime::now();
    }

    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            if let Ok(elapsed) = self.last_accessed.elapsed() {
                return elapsed > ttl;
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct StoredFile {
    pub id: FileId,
    pub metadata: FileMetadata,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub file_id: FileId,
    pub size: u64,
    pub already_exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: FileId,
    pub filename: Option<String>,
    pub content_type: Option<String>,
    pub size: u64,
    pub created_at: SystemTime,
}

impl From<&StoredFile> for FileInfo {
    fn from(file: &StoredFile) -> Self {
        FileInfo {
            id: file.id.clone(),
            filename: file.metadata.filename.clone(),
            content_type: file.metadata.content_type.clone(),
            size: file.metadata.size,
            created_at: file.metadata.created_at,
        }
    }
}

pub fn compute_file_id(content: &[u8]) -> FileId {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content);
    let hash = hasher.finalize();
    FileId(hex::encode(hash))
}
