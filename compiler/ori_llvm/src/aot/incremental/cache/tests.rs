use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
#[expect(
    clippy::disallowed_types,
    reason = "cross-thread test synchronization (Barrier) and shared ownership of the Rust-side test fixture (ArtifactCache), not an Ori-program-visible heap value; Heap<T> does not apply to test-harness thread coordination"
)]
use std::sync::{Arc, Barrier};

fn temp_cache_dir() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("ori_cache_test_{pid}_{id}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

fn sha256_id(digit: char) -> String {
    digit.to_string().repeat(SHA256_HEX_LEN)
}

fn verified_path(
    cache: &ArtifactCache,
    key: &CacheKey,
    object_data: &[u8],
    manifest_data: &[u8],
) -> Option<PathBuf> {
    cache.get_verified(key, |path, manifest| {
        manifest == manifest_data && fs::read(path).is_ok_and(|data| data == object_data)
    })
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
fn artifact_cache_publishes_only_verified_entries() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();
    let key = CacheKey::new(ContentHash::new(1), ContentHash::new(2), &config);
    let object = b"object file content";
    let manifest = b"manifest content";

    assert!(cache
        .get_verified(&key, |_, _| panic!("a missing entry must not be verified"))
        .is_none());

    cache
        .publish(&key, &sha256_id('a'), object, manifest)
        .unwrap();

    assert!(verified_path(&cache, &key, b"wrong object", manifest).is_none());
    assert!(verified_path(&cache, &key, object, b"wrong manifest").is_none());
    let path = verified_path(&cache, &key, object, manifest).unwrap();

    assert_eq!(fs::read(&path).unwrap(), object);
    assert!(is_safe_object_file_name(
        path.file_name().unwrap().to_str().unwrap()
    ));
    cleanup(&dir);
}

#[test]
fn artifact_cache_rejects_non_sha256_object_ids() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();
    let key = CacheKey::new(ContentHash::new(3), ContentHash::new(4), &config);
    let invalid_ids = [
        String::new(),
        "a".repeat(SHA256_HEX_LEN - 1),
        "a".repeat(SHA256_HEX_LEN + 1),
        "A".repeat(SHA256_HEX_LEN),
        "g".repeat(SHA256_HEX_LEN),
        format!("../{}", "a".repeat(SHA256_HEX_LEN)),
    ];

    for object_id in invalid_ids {
        assert!(matches!(
            cache.publish(&key, &object_id, b"object", b"manifest"),
            Err(CacheError::Invalid { .. })
        ));
    }

    assert_eq!(cache.count().unwrap(), 0);
    assert_eq!(cache.size().unwrap(), 0);
    cleanup(&dir);
}

#[test]
fn republishing_same_digest_uses_a_fresh_generation() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();
    let key = CacheKey::new(ContentHash::new(5), ContentHash::new(6), &config);
    let object_id = sha256_id('b');
    let expected = b"expected object";
    let manifest = b"integrity manifest";

    cache.publish(&key, &object_id, expected, manifest).unwrap();
    let first_generation = verified_path(&cache, &key, expected, manifest).unwrap();
    fs::write(&first_generation, b"corrupt object").unwrap();

    assert!(verified_path(&cache, &key, expected, manifest).is_none());
    cache.remove(&key).unwrap();
    cache.publish(&key, &object_id, expected, manifest).unwrap();
    let repaired_generation = verified_path(&cache, &key, expected, manifest).unwrap();

    assert_ne!(first_generation, repaired_generation);
    assert_eq!(fs::read(first_generation).unwrap(), b"corrupt object");
    assert_eq!(fs::read(repaired_generation).unwrap(), expected);
    cleanup(&dir);
}

#[test]
#[expect(
    clippy::disallowed_types,
    reason = "cross-thread test synchronization (Barrier) and shared ownership of the Rust-side test fixture (ArtifactCache), not an Ori-program-visible heap value; Heap<T> does not apply to test-harness thread coordination"
)]
fn concurrent_publication_keeps_object_manifest_pairs_coherent() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = Arc::new(ArtifactCache::new(config.clone()).unwrap());
    let key = CacheKey::new(ContentHash::new(11), ContentHash::new(12), &config);
    let first = vec![b'a'; 1_000_000];
    let second = vec![b'b'; 1_000_000];
    let first_path = dir.join("first.o");
    let second_path = dir.join("second.o");
    fs::write(&first_path, &first).unwrap();
    fs::write(&second_path, &second).unwrap();
    let start = Arc::new(Barrier::new(3));

    let spawn_writer = |source: PathBuf, object_id: String, manifest: &'static [u8]| {
        let cache = Arc::clone(&cache);
        let key = key.clone();
        let start = Arc::clone(&start);
        std::thread::spawn(move || {
            start.wait();
            cache
                .publish_file(&key, &object_id, &source, manifest)
                .unwrap();
        })
    };
    let writer_one = spawn_writer(first_path, sha256_id('c'), b"first manifest");
    let writer_two = spawn_writer(second_path, sha256_id('d'), b"second manifest");
    start.wait();
    writer_one.join().unwrap();
    writer_two.join().unwrap();

    let visible = cache
        .get_verified(&key, |path, manifest| {
            let object = fs::read(path).unwrap();
            (manifest == b"first manifest" && object == first)
                || (manifest == b"second manifest" && object == second)
        })
        .expect("one complete object-manifest pair must be published");

    assert!(fs::read(visible).is_ok_and(|object| object == first || object == second));
    assert_eq!(fs::read_dir(dir.join("objects")).unwrap().count(), 2);
    cleanup(&dir);
}

#[test]
fn entry_envelope_rejects_unsafe_object_paths() {
    let malformed_names = [
        "../outside.o",
        "..\\outside.o",
        "/tmp/outside.o",
        "C:\\outside.o",
        "not-an-object.o",
    ];
    for file_name in malformed_names {
        let envelope = encode_entry_envelope(file_name, b"manifest");
        assert!(decode_entry_envelope(&envelope).is_none());
    }

    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();
    let key = CacheKey::new(ContentHash::new(13), ContentHash::new(14), &config);
    let outside = dir.join("outside.o");
    fs::write(&outside, b"outside object").unwrap();
    fs::write(
        cache.meta_path(&key),
        encode_entry_envelope("../outside.o", b"manifest"),
    )
    .unwrap();

    assert!(cache
        .get_verified(&key, |_, _| panic!("an unsafe path reached the verifier"))
        .is_none());
    cache.remove(&key).unwrap();
    assert_eq!(fs::read(outside).unwrap(), b"outside object");
    cleanup(&dir);
}

#[test]
fn artifact_cache_remove_preserves_active_reader_generation() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();
    let key = CacheKey::new(ContentHash::new(15), ContentHash::new(16), &config);
    cache
        .publish(&key, &sha256_id('e'), b"object", b"manifest")
        .unwrap();
    let active_reader_path = verified_path(&cache, &key, b"object", b"manifest").unwrap();

    cache.remove(&key).unwrap();

    assert!(cache.get_verified(&key, |_, _| true).is_none());
    assert_eq!(fs::read(&active_reader_path).unwrap(), b"object");
    cache.clear().unwrap();
    assert!(!active_reader_path.exists());
    cleanup(&dir);
}

#[test]
fn test_artifact_cache_clear() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();
    let object_id = sha256_id('f');

    for i in 0..5 {
        let key = CacheKey::new(ContentHash::new(i), ContentHash::new(0), &config);
        cache
            .publish(&key, &object_id, b"data", b"manifest")
            .unwrap();
    }

    assert_eq!(cache.count().unwrap(), 5);
    cache.clear().unwrap();
    assert_eq!(cache.count().unwrap(), 0);
    assert_eq!(cache.size().unwrap(), 0);
    cleanup(&dir);
}

#[test]
fn test_artifact_cache_size() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir);
    let cache = ArtifactCache::new(config.clone()).unwrap();
    let key = CacheKey::new(ContentHash::new(1), ContentHash::new(2), &config);
    let data = vec![0u8; 1024];

    cache
        .publish(&key, &sha256_id('1'), &data, b"manifest")
        .unwrap();

    assert_eq!(cache.size().unwrap(), 1024);
    cleanup(&dir);
}

#[test]
fn test_artifact_cache_validate() {
    let dir = temp_cache_dir();
    let config = CacheConfig::new(&dir).with_version("1.0.0");
    let cache = ArtifactCache::new(config).unwrap();

    assert!(cache.validate().unwrap());
    fs::write(dir.join("version"), b"2.0.0").unwrap();
    assert!(!cache.validate().unwrap());
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
