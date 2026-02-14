use bytes::Bytes;
use faber_store::{FileMetadata, FileStore, FilesystemStore, StoreConfig};
use tempfile::TempDir;

#[tokio::test]
async fn test_filesystem_put_and_get() {
    let temp_dir = TempDir::new().unwrap();
    let store = FilesystemStore::new(
        temp_dir.path().to_string_lossy().to_string(),
        StoreConfig::default(),
    );

    let content = Bytes::from("Hello, Filesystem!");
    let metadata = FileMetadata::new(content.len() as u64)
        .with_filename("test.txt")
        .with_content_type("text/plain");

    let result = store.put(content.clone(), metadata).await.unwrap();

    assert!(!result.already_exists);
    assert_eq!(result.size, 18);

    let retrieved = store.get(&result.file_id).await.unwrap();
    assert_eq!(retrieved.content, &content[..]);
    assert_eq!(retrieved.metadata.filename, Some("test.txt".to_string()));
}

#[tokio::test]
async fn test_filesystem_put_duplicate() {
    let temp_dir = TempDir::new().unwrap();
    let store = FilesystemStore::new(
        temp_dir.path().to_string_lossy().to_string(),
        StoreConfig::default(),
    );

    let content = Bytes::from("Same content");
    let metadata = FileMetadata::new(content.len() as u64);

    let result1 = store.put(content.clone(), metadata.clone()).await.unwrap();
    let result2 = store.put(content.clone(), metadata).await.unwrap();

    assert!(!result1.already_exists);
    assert!(result2.already_exists);
    assert_eq!(result1.file_id, result2.file_id);
}

#[tokio::test]
async fn test_filesystem_exists() {
    let temp_dir = TempDir::new().unwrap();
    let store = FilesystemStore::new(
        temp_dir.path().to_string_lossy().to_string(),
        StoreConfig::default(),
    );

    let content = Bytes::from("Test content");
    let metadata = FileMetadata::new(content.len() as u64);

    let result = store.put(content, metadata).await.unwrap();

    assert!(store.exists(&result.file_id).await.unwrap());
    let fake_id = faber_store::compute_file_id(b"nonexistent");
    assert!(!store.exists(&fake_id).await.unwrap());
}

#[tokio::test]
async fn test_filesystem_delete() {
    let temp_dir = TempDir::new().unwrap();
    let store = FilesystemStore::new(
        temp_dir.path().to_string_lossy().to_string(),
        StoreConfig::default(),
    );

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
async fn test_filesystem_list() {
    let temp_dir = TempDir::new().unwrap();
    let store = FilesystemStore::new(
        temp_dir.path().to_string_lossy().to_string(),
        StoreConfig::default(),
    );

    let content1 = Bytes::from("File 1");
    let content2 = Bytes::from("File 2");

    let result1 = store
        .put(
            content1,
            FileMetadata::new(6).with_filename("file1.txt"),
        )
        .await
        .unwrap();
    let result2 = store
        .put(
            content2,
            FileMetadata::new(6).with_filename("file2.txt"),
        )
        .await
        .unwrap();

    let files = store.list().await.unwrap();
    assert_eq!(files.len(), 2);

    let ids: Vec<_> = files.iter().map(|f| f.id.clone()).collect();
    assert!(ids.contains(&result1.file_id));
    assert!(ids.contains(&result2.file_id));
}

#[tokio::test]
async fn test_filesystem_touch() {
    let temp_dir = TempDir::new().unwrap();
    let store = FilesystemStore::new(
        temp_dir.path().to_string_lossy().to_string(),
        StoreConfig::default(),
    );

    let content = Bytes::from("Touch test");
    let metadata = FileMetadata::new(content.len() as u64);

    let result = store.put(content, metadata).await.unwrap();
    let original = store.get_metadata(&result.file_id).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    store.touch(&result.file_id).await.unwrap();
    let updated = store.get_metadata(&result.file_id).await.unwrap();

    assert!(updated.last_accessed > original.last_accessed);
}

#[tokio::test]
async fn test_filesystem_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_string_lossy().to_string();

    let content = Bytes::from("Persistent content");

    let file_id = {
        let store = FilesystemStore::new(path.clone(), StoreConfig::default());
        let result = store
            .put(content.clone(), FileMetadata::new(content.len() as u64))
            .await
            .unwrap();
        result.file_id
    };

    let store2 = FilesystemStore::new(path, StoreConfig::default());
    let retrieved = store2.get(&file_id).await.unwrap();
    assert_eq!(retrieved.content, &content[..]);
}
