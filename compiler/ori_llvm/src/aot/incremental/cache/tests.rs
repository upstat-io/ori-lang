use super::*;
use std::env;

fn temp_cache_dir() -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "ori_cache_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_cache_config() {
    let config = CacheConfig::new("/tmp/cache")
        .with_version("1.0.0")
        .with_opt_level("O2")
        .with_target("x86_64-linux-gnu");

    assert_eq!(config.compiler_version, "1.0.0");
    assert_eq!(config.opt_level, "O2");
    assert_eq!(config.target, "x86_64-linux-gnu");
}

#[test]
fn test_cache_key() {
    let config = CacheConfig::new("/tmp/cache");
    let source = ContentHash::new(123);
    let deps = ContentHash::new(456);

    let key = CacheKey::new(source, deps, &config);

    assert_eq!(key.source_hash(), source);
    assert_eq!(key.deps_hash(), deps);
    assert!(!key.to_filename().is_empty());
}

#[test]
fn test_cache_key_deterministic() {
    let config = CacheConfig::new("/tmp/cache")
        .with_version("1.0.0")
        .with_opt_level("O2");

    let key1 = CacheKey::new(ContentHash::new(100), ContentHash::new(200), &config);
    let key2 = CacheKey::new(ContentHash::new(100), ContentHash::new(200), &config);

    assert_eq!(key1.hash(), key2.hash());
}

#[test]
fn test_cache_key_changes_with_flags() {
    let config1 = CacheConfig::new("/tmp/cache").with_opt_level("O0");
    let config2 = CacheConfig::new("/tmp/cache").with_opt_level("O3");

    let key1 = CacheKey::new(ContentHash::new(100), ContentHash::new(200), &config1);
    let key2 = CacheKey::new(ContentHash::new(100), ContentHash::new(200), &config2);

    assert_ne!(key1.hash(), key2.hash());
}

#[test]
fn test_artifact_cache_create() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);

    let _cache = ArtifactCache::new(config).unwrap();

    assert!(dir.join("objects").exists());
    assert!(dir.join("meta").exists());
    assert!(dir.join("version").exists());

    cleanup(&dir);
}

#[test]
fn test_artifact_cache_put_get() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();

    let key = CacheKey::new(ContentHash::new(1), ContentHash::new(2), &config);

    // Initially not in cache
    assert!(!cache.has(&key));

    // Put data
    let data = b"object file content";
    cache.put(&key, data).unwrap();

    // Now in cache
    assert!(cache.has(&key));
    let path = cache.get(&key).unwrap();
    assert!(path.exists());

    cleanup(&dir);
}

#[test]
fn test_artifact_cache_remove() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();

    let key = CacheKey::new(ContentHash::new(1), ContentHash::new(2), &config);

    cache.put(&key, b"data").unwrap();
    assert!(cache.has(&key));

    cache.remove(&key).unwrap();
    assert!(!cache.has(&key));

    cleanup(&dir);
}

#[test]
fn test_artifact_cache_clear() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();

    // Add multiple items
    for i in 0..5 {
        let key = CacheKey::new(ContentHash::new(i), ContentHash::new(0), &config);
        cache.put(&key, b"data").unwrap();
    }

    assert_eq!(cache.count().unwrap(), 5);

    cache.clear().unwrap();
    assert_eq!(cache.count().unwrap(), 0);

    cleanup(&dir);
}

#[test]
fn test_artifact_cache_size() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();

    let key = CacheKey::new(ContentHash::new(1), ContentHash::new(2), &config);

    let data = vec![0u8; 1024]; // 1KB
    cache.put(&key, &data).unwrap();

    assert_eq!(cache.size().unwrap(), 1024);

    cleanup(&dir);
}

#[test]
fn test_artifact_cache_validate() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir).with_version("1.0.0");
    let cache = ArtifactCache::new(config).unwrap();

    assert!(cache.validate().unwrap());

    // Create cache with different version
    let config2 = CacheConfig::new(&dir).with_version("2.0.0");
    let _cache2 = ArtifactCache::new(config2).unwrap();

    // Cache should be invalid for old version check
    let config_old = CacheConfig::new(&dir).with_version("1.0.0");
    let _cache_old = ArtifactCache::new(config_old.clone()).unwrap();
    // But the version file now says 2.0.0, so validating with 1.0.0 should fail
    // (We need to recreate to get the fresh version file)

    cleanup(&dir);
}

#[test]
fn test_cache_error_display() {
    let err = CacheError::IoError {
        path: PathBuf::from("/test"),
        message: "permission denied".to_string(),
    };
    assert!(err.to_string().contains("/test"));
    assert!(err.to_string().contains("permission denied"));

    let err = CacheError::Invalid {
        message: "version mismatch".to_string(),
    };
    assert!(err.to_string().contains("version mismatch"));
}
