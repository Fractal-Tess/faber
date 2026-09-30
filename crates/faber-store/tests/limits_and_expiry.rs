#![cfg(all(feature = "memory", feature = "filesystem"))]

use bytes::Bytes;
use faber_store::{
    FileMetadata, FileStore, FilesystemStore, HybridStore, MemoryStore, StoreConfig, StoreError,
};
use std::{sync::Arc, time::Duration};
use tempfile::TempDir;

fn config() -> faber_store::StoreConfigBuilder {
    StoreConfig::builder()
        .max_file_size(1024)
        .max_total_bytes(100)
        .max_entries(3)
}

/// Every backend, each with its own temporary directory kept alive.
fn stores(config: StoreConfig) -> Vec<(&'static str, Arc<dyn FileStore>, Option<TempDir>)> {
    let filesystem_dir = TempDir::new().unwrap();
    let hybrid_dir = TempDir::new().unwrap();
    vec![
        ("memory", Arc::new(MemoryStore::new(config.clone())), None),
        (
            "filesystem",
            Arc::new(FilesystemStore::new(
                filesystem_dir.path().to_string_lossy().to_string(),
                config.clone(),
            )),
            Some(filesystem_dir),
        ),
        (
            "hybrid",
            Arc::new(HybridStore::new(
                hybrid_dir.path().to_string_lossy().to_string(),
                10,
                1024,
                config,
            )),
            Some(hybrid_dir),
        ),
    ]
}

async fn put(
    store: &dyn FileStore,
    content: &str,
) -> Result<faber_store::UploadResult, StoreError> {
    store
        .put(
            Bytes::from(content.to_string()),
            FileMetadata::new(content.len() as u64),
        )
        .await
}

#[tokio::test]
async fn every_backend_enforces_total_bytes_and_entries() {
    for (name, store, _dir) in stores(config().build()) {
        let first = put(store.as_ref(), &"a".repeat(60)).await.unwrap();
        assert!(
            matches!(
                put(store.as_ref(), &"b".repeat(60)).await,
                Err(StoreError::StoreFull(_))
            ),
            "{name}: accepted bytes beyond max_total_bytes"
        );
        // Re-uploading stored content needs no new space.
        assert!(
            put(store.as_ref(), &"a".repeat(60))
                .await
                .unwrap()
                .already_exists
        );

        put(store.as_ref(), "c").await.unwrap();
        put(store.as_ref(), "d").await.unwrap();
        assert!(
            matches!(
                put(store.as_ref(), "e").await,
                Err(StoreError::StoreFull(_))
            ),
            "{name}: accepted entries beyond max_entries"
        );

        assert!(store.delete(&first.file_id).await.unwrap());
        put(store.as_ref(), &"b".repeat(60))
            .await
            .unwrap_or_else(|error| panic!("{name}: delete did not free space: {error}"));
    }
}

#[tokio::test]
async fn every_backend_expires_files_on_read_and_by_sweep() {
    let config = config().default_ttl(Duration::from_millis(200)).build();
    for (name, store, _dir) in stores(config) {
        let read_expired = put(store.as_ref(), "read-expired").await.unwrap();
        let swept = put(store.as_ref(), "swept").await.unwrap();
        let kept = put(store.as_ref(), "kept").await.unwrap();

        for _ in 0..4 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            store.get(&kept.file_id).await.unwrap();
        }

        assert!(
            matches!(
                store.get(&read_expired.file_id).await,
                Err(StoreError::NotFound(_))
            ),
            "{name}: expired file was still readable"
        );
        assert!(
            !store.exists(&swept.file_id).await.unwrap(),
            "{name}: expired file still exists"
        );
        assert_eq!(
            store.purge_expired().await.unwrap(),
            0,
            "{name}: expired files survived reads"
        );
        let listed: Vec<_> = store
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|f| f.id)
            .collect();
        assert_eq!(listed, vec![kept.file_id.clone()], "{name}: listing");

        // Expired files no longer count against the quota.
        put(store.as_ref(), &"z".repeat(90))
            .await
            .unwrap_or_else(|error| panic!("{name}: expired files still use quota: {error}"));
    }
}

#[tokio::test]
async fn background_sweep_removes_expired_files() {
    let config = config().default_ttl(Duration::from_millis(100)).build();
    for (name, store, _dir) in stores(config) {
        put(store.as_ref(), "one").await.unwrap();
        put(store.as_ref(), "two").await.unwrap();
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(store.purge_expired().await.unwrap(), 2, "{name}");
        put(store.as_ref(), "three").await.unwrap();
        put(store.as_ref(), "four").await.unwrap();
        put(store.as_ref(), "five")
            .await
            .unwrap_or_else(|error| panic!("{name}: swept files still counted: {error}"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_filesystem_reads_leave_metadata_intact() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(FilesystemStore::new(
        dir.path().to_string_lossy().to_string(),
        StoreConfig::default(),
    ));
    let content = "x".repeat(4096);
    let id = store
        .put(
            Bytes::from(content.clone()),
            FileMetadata::new(content.len() as u64).with_filename("file-with-a-long-name.txt"),
        )
        .await
        .unwrap()
        .file_id;

    let readers: Vec<_> = (0..64)
        .map(|_| {
            let store = store.clone();
            let id = id.clone();
            tokio::spawn(async move {
                for _ in 0..10 {
                    store.get(&id).await.expect("concurrent read failed");
                }
            })
        })
        .collect();
    for reader in readers {
        reader.await.unwrap();
    }

    let metadata = store.get_metadata(&id).await.expect("metadata was torn");
    assert_eq!(
        metadata.filename.as_deref(),
        Some("file-with-a-long-name.txt")
    );
}
