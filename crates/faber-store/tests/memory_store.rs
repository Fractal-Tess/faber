use bytes::Bytes;
use faber_store::{FileMetadata, FileStore, MemoryStore, StoreConfig, compute_file_id};
use std::time::Duration;

#[tokio::test]
async fn test_put_and_get() {
    let store = MemoryStore::new(StoreConfig::default());
    let content = Bytes::from("Hello, World!");
    let metadata = FileMetadata::new(content.len() as u64)
        .with_filename("test.txt")
        .with_content_type("text/plain");

    let result = store.put(content.clone(), metadata).await.unwrap();

    assert!(!result.already_exists);
    assert_eq!(result.size, 13);

    let retrieved = store.get(&result.file_id).await.unwrap();
    assert_eq!(retrieved.content, &content[..]);
    assert_eq!(retrieved.metadata.filename, Some("test.txt".to_string()));
}

#[tokio::test]
async fn test_put_duplicate() {
    let store = MemoryStore::new(StoreConfig::default());
    let content = Bytes::from("Same content");
    let metadata = FileMetadata::new(content.len() as u64);

    let result1 = store.put(content.clone(), metadata.clone()).await.unwrap();
    let result2 = store.put(content.clone(), metadata).await.unwrap();

    assert!(!result1.already_exists);
    assert!(result2.already_exists);
    assert_eq!(result1.file_id, result2.file_id);
}

#[tokio::test]
async fn test_exists() {
    let store = MemoryStore::new(StoreConfig::default());
    let content = Bytes::from("Test content");
    let metadata = FileMetadata::new(content.len() as u64);

    let result = store.put(content, metadata).await.unwrap();

    assert!(store.exists(&result.file_id).await.unwrap());
    assert!(
        !store
            .exists(&compute_file_id(b"nonexistent"))
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn test_delete() {
    let store = MemoryStore::new(StoreConfig::default());
    let content = Bytes::from("To be deleted");
    let metadata = FileMetadata::new(content.len() as u64);

    let result = store.put(content, metadata).await.unwrap();
    assert!(store.exists(&result.file_id).await.unwrap());

    let deleted = store.delete(&result.file_id).await.unwrap();
    assert!(deleted);
    assert!(!store.exists(&result.file_id).await.unwrap());

    let deleted_again = store.delete(&result.file_id).await.unwrap();
    assert!(!deleted_again);
}

#[tokio::test]
async fn test_list() {
    let store = MemoryStore::new(StoreConfig::default());

    let content1 = Bytes::from("File 1");
    let content2 = Bytes::from("File 2");

    let result1 = store
        .put(content1, FileMetadata::new(6).with_filename("file1.txt"))
        .await
        .unwrap();
    let result2 = store
        .put(content2, FileMetadata::new(6).with_filename("file2.txt"))
        .await
        .unwrap();

    let files = store.list().await.unwrap();
    assert_eq!(files.len(), 2);

    let ids: Vec<_> = files.iter().map(|f| f.id.clone()).collect();
    assert!(ids.contains(&result1.file_id));
    assert!(ids.contains(&result2.file_id));
}

#[tokio::test]
async fn test_touch() {
    let store = MemoryStore::new(StoreConfig::default());
    let content = Bytes::from("Touch test");
    let metadata = FileMetadata::new(content.len() as u64);

    let result = store.put(content, metadata).await.unwrap();
    let original = store.get_metadata(&result.file_id).await.unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;

    store.touch(&result.file_id).await.unwrap();
    let updated = store.get_metadata(&result.file_id).await.unwrap();

    assert!(updated.last_accessed > original.last_accessed);
}

#[tokio::test]
async fn test_get_nonexistent() {
    let store = MemoryStore::new(StoreConfig::default());
    let fake_id = compute_file_id(b"nonexistent");

    let result = store.get(&fake_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_too_large() {
    let config = StoreConfig::builder().memory().max_file_size(10).build();
    let store = MemoryStore::new(config);
    let content = Bytes::from("This is more than 10 bytes");
    let metadata = FileMetadata::new(content.len() as u64);

    let result = store.put(content, metadata).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_compute_file_id_consistency() {
    let content1 = b"same content";
    let content2 = b"same content";
    let content3 = b"different content";

    let id1 = compute_file_id(content1);
    let id2 = compute_file_id(content2);
    let id3 = compute_file_id(content3);

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[tokio::test]
async fn test_file_id_is_sha256() {
    let content = b"test";
    let id = compute_file_id(content);

    // SHA256 of "test" is known
    assert_eq!(
        id.as_str(),
        "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
    );
}
