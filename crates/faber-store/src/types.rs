use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::time::{Duration, SystemTime};

use crate::{StoreError, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileId(String);

impl FileId {
    pub fn new(value: impl Into<String>) -> StoreResult<Self> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StoreError::InvalidFileId(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for FileId {
    type Error = StoreError;

    fn try_from(value: String) -> StoreResult<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for FileId {
    type Error = StoreError;

    fn try_from(value: &str) -> StoreResult<Self> {
        Self::new(value)
    }
}

impl Serialize for FileId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for FileId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
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

#[cfg(test)]
mod tests {
    use super::FileId;

    #[test]
    fn file_id_rejects_short_and_traversal_values() {
        for value in ["abc", "../../etc/passwd", "../" , "éé"] {
            assert!(FileId::new(value).is_err(), "accepted invalid id {value:?}");
        }
    }

    #[test]
    fn file_id_requires_lowercase_sha256_hex() {
        assert!(FileId::new("A".repeat(64)).is_err());
        assert!(FileId::new("g".repeat(64)).is_err());
        assert!(FileId::new("a".repeat(64)).is_ok());
    }
}
