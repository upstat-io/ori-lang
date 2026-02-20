use super::*;
use std::io::Write;

fn create_temp_file(content: &str) -> PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("ori_hash_test_{}.ori", rand_suffix()));
    let mut file = File::create(&path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    path
}

fn rand_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

#[test]
fn test_content_hash_display() {
    let hash = ContentHash::new(0x1234_5678_9abc_def0);
    assert_eq!(hash.to_string(), "123456789abcdef0");
}

#[test]
fn test_content_hash_from_hex() {
    let hash = ContentHash::from_hex("123456789abcdef0").unwrap();
    assert_eq!(hash.value(), 0x1234_5678_9abc_def0);
}

#[test]
fn test_content_hash_from_hex_invalid() {
    assert!(ContentHash::from_hex("not_hex").is_none());
}

#[test]
fn test_hash_string() {
    let h1 = hash_string("hello");
    let h2 = hash_string("hello");
    let h3 = hash_string("world");

    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn test_combine_hashes() {
    let h1 = hash_string("a");
    let h2 = hash_string("b");

    let combined1 = combine_hashes(&[h1, h2]);
    let combined2 = combine_hashes(&[h1, h2]);
    let combined3 = combine_hashes(&[h2, h1]); // Different order

    assert_eq!(combined1, combined2);
    assert_ne!(combined1, combined3); // Order matters
}

#[test]
fn test_source_hasher_file() {
    let path = create_temp_file("@main () -> void = print(msg: \"hello\");");
    let mut hasher = SourceHasher::new();

    let hash1 = hasher.hash_file(&path).unwrap();
    let hash2 = hasher.hash_file(&path).unwrap();

    assert_eq!(hash1, hash2);

    // Clean up
    let _ = fs::remove_file(&path);
}

#[test]
fn test_source_hasher_caching() {
    let path = create_temp_file("let x = 42");
    let mut hasher = SourceHasher::new();

    // First hash - computed fresh
    let _ = hasher.hash_file(&path).unwrap();
    assert!(hasher.get_cached(&path).is_some());

    // Second hash - should use cache
    let hash2 = hasher.hash_file(&path).unwrap();
    assert_eq!(hasher.get_cached(&path).unwrap().content_hash, hash2);

    let _ = fs::remove_file(&path);
}

#[test]
fn test_source_hasher_change_detection() {
    let path = create_temp_file("version 1");
    let mut hasher = SourceHasher::new();

    let hash1 = hasher.hash_file(&path).unwrap();

    // Modify the file
    std::thread::sleep(std::time::Duration::from_millis(10));
    let mut file = File::create(&path).unwrap();
    file.write_all(b"version 2").unwrap();

    // Clear cache to force recomputation
    hasher.clear_cache();
    let hash2 = hasher.hash_file(&path).unwrap();

    assert_ne!(hash1, hash2);

    let _ = fs::remove_file(&path);
}

#[test]
fn test_source_hasher_has_changed() {
    let path = create_temp_file("original content");
    let mut hasher = SourceHasher::new();

    // Initial hash - populate the cache
    let _ = hasher.hash_file(&path).unwrap();

    // Should not have changed (same content, same metadata)
    // Don't clear cache - we need the old hash for comparison
    assert!(!hasher.has_changed(&path).unwrap());

    // Modify file
    std::thread::sleep(std::time::Duration::from_millis(10));
    let mut file = File::create(&path).unwrap();
    file.write_all(b"modified content").unwrap();
    drop(file);

    // Now it should show as changed
    assert!(hasher.has_changed(&path).unwrap());

    let _ = fs::remove_file(&path);
}

#[test]
fn test_hash_multiple_files() {
    let path1 = create_temp_file("file 1");
    let path2 = create_temp_file("file 2");

    let mut hasher = SourceHasher::new();
    let combined = hasher.hash_files(&[path1.clone(), path2.clone()]).unwrap();

    // Same files in same order should give same hash
    hasher.clear_cache();
    let combined2 = hasher.hash_files(&[path1.clone(), path2.clone()]).unwrap();
    assert_eq!(combined, combined2);

    let _ = fs::remove_file(&path1);
    let _ = fs::remove_file(&path2);
}

#[test]
fn test_hash_error_display() {
    let err = HashError::IoError {
        path: PathBuf::from("/test/file.ori"),
        message: "permission denied".to_string(),
    };
    assert!(err.to_string().contains("/test/file.ori"));
    assert!(err.to_string().contains("permission denied"));

    let err = HashError::NotFound {
        path: PathBuf::from("/missing.ori"),
    };
    assert!(err.to_string().contains("/missing.ori"));
}

#[test]
fn test_normalized_hashing() {
    let path1 = create_temp_file("let x = 1\nlet y = 2");
    let path2 = create_temp_file("let x = 1  \nlet y = 2  "); // Trailing whitespace

    let mut hasher = SourceHasher::new().with_normalization(true);

    let hash1 = hasher.hash_file(&path1).unwrap();
    hasher.clear_cache();
    let hash2 = hasher.hash_file(&path2).unwrap();

    // With normalization, trailing whitespace should be ignored
    assert_eq!(hash1, hash2);

    let _ = fs::remove_file(&path1);
    let _ = fs::remove_file(&path2);
}

#[test]
fn test_fx_hasher_deterministic() {
    let mut h1 = FxHasher::default();
    let mut h2 = FxHasher::default();

    h1.write(b"test data");
    h2.write(b"test data");

    assert_eq!(h1.finish(), h2.finish());
}

#[test]
fn test_file_metadata_might_be_unchanged() {
    let meta1 = FileMetadata {
        size: 100,
        mtime: SystemTime::UNIX_EPOCH,
        content_hash: ContentHash::new(123),
    };

    let meta2 = FileMetadata {
        size: 100,
        mtime: SystemTime::UNIX_EPOCH,
        content_hash: ContentHash::new(456), // Different hash but same metadata
    };

    let meta3 = FileMetadata {
        size: 200, // Different size
        mtime: SystemTime::UNIX_EPOCH,
        content_hash: ContentHash::new(123),
    };

    assert!(meta1.might_be_unchanged(&meta2));
    assert!(!meta1.might_be_unchanged(&meta3));
}
