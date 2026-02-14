mod backends;
mod config;
mod error;
mod lru;
mod store;
mod types;

pub use config::{BackendConfig, StoreConfig, StoreConfigBuilder};
pub use error::{StoreError, StoreResult};
pub use lru::LruCache;
pub use store::FileStore;
pub use types::{compute_file_id, FileInfo, FileId, FileMetadata, StoredFile, UploadResult};

#[cfg(feature = "memory")]
pub use backends::MemoryStore;

#[cfg(feature = "filesystem")]
pub use backends::FilesystemStore;

#[cfg(all(feature = "memory", feature = "filesystem"))]
pub use backends::HybridStore;

pub fn create_store(config: StoreConfig) -> std::sync::Arc<dyn FileStore> {
    use std::sync::Arc;

    match &config.backend {
        #[cfg(feature = "memory")]
        config::BackendConfig::Memory => Arc::new(backends::MemoryStore::new(config)),

        #[cfg(not(feature = "memory"))]
        config::BackendConfig::Memory => {
            panic!("Memory backend not compiled in. Enable 'memory' feature.")
        }

        #[cfg(feature = "filesystem")]
        config::BackendConfig::Filesystem { path } => {
            Arc::new(backends::FilesystemStore::new(path.clone(), config))
        }

        #[cfg(not(feature = "filesystem"))]
        config::BackendConfig::Filesystem { .. } => {
            panic!("Filesystem backend not compiled in. Enable 'filesystem' feature.")
        }

        #[cfg(all(feature = "memory", feature = "filesystem"))]
        config::BackendConfig::Hybrid { path, max_memory_entries, max_memory_size } => {
            Arc::new(backends::HybridStore::new(
                path.clone(),
                *max_memory_entries,
                *max_memory_size,
                config,
            ))
        }

        #[cfg(not(all(feature = "memory", feature = "filesystem")))]
        config::BackendConfig::Hybrid { .. } => {
            panic!("Hybrid backend requires both 'memory' and 'filesystem' features.")
        }
    }
}
