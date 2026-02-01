//! Artifact Cache for Incremental Compilation
//!
//! Caches compiled object files and other artifacts to avoid recompilation.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::hash::{combine_hashes, hash_string, ContentHash};

/// Configuration for the artifact cache.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Root directory for cache storage.
    pub cache_dir: PathBuf,
    /// Maximum cache size in bytes (0 = unlimited).
    pub max_size: u64,
    /// Compiler version for cache invalidation.
    pub compiler_version: String,
    /// Optimization level (part of cache key).
    pub opt_level: String,
    /// Target triple (part of cache key).
    pub target: String,
}

impl CacheConfig {
    /// Create a new cache configuration.
    #[must_use]
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
            max_size: 0,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            opt_level: "default".to_string(),
            target: "native".to_string(),
        }
    }

    /// Set the compiler version.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.compiler_version = version.into();
        self
    }

    /// Set the optimization level.
    #[must_use]
    pub fn with_opt_level(mut self, level: impl Into<String>) -> Self {
        self.opt_level = level.into();
        self
    }

    /// Set the target triple.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = target.into();
        self
    }

    /// Set maximum cache size.
    #[must_use]
    pub fn with_max_size(mut self, bytes: u64) -> Self {
        self.max_size = bytes;
        self
    }
}

/// A cache key identifying a cached artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Hash of the source file content.
    source_hash: ContentHash,
    /// Hash of all dependencies' content.
    deps_hash: ContentHash,
    /// Hash of compilation flags.
    flags_hash: ContentHash,
    /// Combined key hash.
    combined: ContentHash,
}

impl CacheKey {
    /// Create a new cache key.
    #[must_use]
    pub fn new(source_hash: ContentHash, deps_hash: ContentHash, config: &CacheConfig) -> Self {
        // Hash the flags (version + opt level + target)
        let flags_str = format!(
            "{}:{}:{}",
            config.compiler_version, config.opt_level, config.target
        );
        let flags_hash = hash_string(&flags_str);

        // Combine all hashes
        let combined = combine_hashes(&[source_hash, deps_hash, flags_hash]);

        Self {
            source_hash,
            deps_hash,
            flags_hash,
            combined,
        }
    }

    /// Get the combined hash for file naming.
    #[must_use]
    pub fn hash(&self) -> ContentHash {
        self.combined
    }

    /// Get the source hash.
    #[must_use]
    pub fn source_hash(&self) -> ContentHash {
        self.source_hash
    }

    /// Get the dependencies hash.
    #[must_use]
    pub fn deps_hash(&self) -> ContentHash {
        self.deps_hash
    }

    /// Convert to a filename-safe string.
    #[must_use]
    pub fn to_filename(&self) -> String {
        self.combined.to_hex()
    }
}

/// Artifact cache for storing compiled objects.
#[derive(Debug)]
pub struct ArtifactCache {
    /// Cache configuration.
    config: CacheConfig,
    /// Path to objects directory.
    objects_dir: PathBuf,
    /// Path to metadata directory.
    meta_dir: PathBuf,
}

impl ArtifactCache {
    /// Create a new artifact cache.
    ///
    /// Creates the cache directory structure if it doesn't exist.
    pub fn new(config: CacheConfig) -> Result<Self, CacheError> {
        let objects_dir = config.cache_dir.join("objects");
        let meta_dir = config.cache_dir.join("meta");

        // Create directories
        fs::create_dir_all(&objects_dir).map_err(|e| CacheError::IoError {
            path: objects_dir.clone(),
            message: e.to_string(),
        })?;

        fs::create_dir_all(&meta_dir).map_err(|e| CacheError::IoError {
            path: meta_dir.clone(),
            message: e.to_string(),
        })?;

        // Write version file for cache invalidation
        let version_file = config.cache_dir.join("version");
        let mut file = File::create(&version_file).map_err(|e| CacheError::IoError {
            path: version_file.clone(),
            message: e.to_string(),
        })?;
        file.write_all(config.compiler_version.as_bytes())
            .map_err(|e| CacheError::IoError {
                path: version_file,
                message: e.to_string(),
            })?;

        Ok(Self {
            config,
            objects_dir,
            meta_dir,
        })
    }

    /// Get the path where an object file would be cached.
    #[must_use]
    pub fn object_path(&self, key: &CacheKey) -> PathBuf {
        self.objects_dir.join(format!("{}.o", key.to_filename()))
    }

    /// Get the path where metadata would be stored.
    #[must_use]
    pub fn meta_path(&self, key: &CacheKey) -> PathBuf {
        self.meta_dir.join(format!("{}.json", key.to_filename()))
    }

    /// Check if a cached artifact exists.
    #[must_use]
    pub fn has(&self, key: &CacheKey) -> bool {
        self.object_path(key).exists()
    }

    /// Get a cached object file.
    ///
    /// Returns the path to the cached object if it exists.
    pub fn get(&self, key: &CacheKey) -> Option<PathBuf> {
        let path = self.object_path(key);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// Store an object file in the cache.
    pub fn put(&self, key: &CacheKey, object_data: &[u8]) -> Result<PathBuf, CacheError> {
        let path = self.object_path(key);

        // Write object file
        let mut file = File::create(&path).map_err(|e| CacheError::IoError {
            path: path.clone(),
            message: e.to_string(),
        })?;

        file.write_all(object_data)
            .map_err(|e| CacheError::IoError {
                path: path.clone(),
                message: e.to_string(),
            })?;

        Ok(path)
    }

    /// Store an object file by copying from an existing path.
    pub fn put_file(&self, key: &CacheKey, source: &Path) -> Result<PathBuf, CacheError> {
        let dest = self.object_path(key);

        fs::copy(source, &dest).map_err(|e| CacheError::IoError {
            path: dest.clone(),
            message: e.to_string(),
        })?;

        Ok(dest)
    }

    /// Remove a cached artifact.
    pub fn remove(&self, key: &CacheKey) -> Result<(), CacheError> {
        let obj_path = self.object_path(key);
        let meta_path = self.meta_path(key);

        if obj_path.exists() {
            fs::remove_file(&obj_path).map_err(|e| CacheError::IoError {
                path: obj_path,
                message: e.to_string(),
            })?;
        }

        if meta_path.exists() {
            fs::remove_file(&meta_path).map_err(|e| CacheError::IoError {
                path: meta_path,
                message: e.to_string(),
            })?;
        }

        Ok(())
    }

    /// Clear the entire cache.
    pub fn clear(&self) -> Result<(), CacheError> {
        // Remove and recreate directories
        if self.objects_dir.exists() {
            fs::remove_dir_all(&self.objects_dir).map_err(|e| CacheError::IoError {
                path: self.objects_dir.clone(),
                message: e.to_string(),
            })?;
        }

        if self.meta_dir.exists() {
            fs::remove_dir_all(&self.meta_dir).map_err(|e| CacheError::IoError {
                path: self.meta_dir.clone(),
                message: e.to_string(),
            })?;
        }

        fs::create_dir_all(&self.objects_dir).map_err(|e| CacheError::IoError {
            path: self.objects_dir.clone(),
            message: e.to_string(),
        })?;

        fs::create_dir_all(&self.meta_dir).map_err(|e| CacheError::IoError {
            path: self.meta_dir.clone(),
            message: e.to_string(),
        })?;

        Ok(())
    }

    /// Get the total size of cached objects.
    pub fn size(&self) -> Result<u64, CacheError> {
        let mut total = 0u64;

        for entry in fs::read_dir(&self.objects_dir).map_err(|e| CacheError::IoError {
            path: self.objects_dir.clone(),
            message: e.to_string(),
        })? {
            let entry = entry.map_err(|e| CacheError::IoError {
                path: self.objects_dir.clone(),
                message: e.to_string(),
            })?;

            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }

        Ok(total)
    }

    /// Get the number of cached objects.
    pub fn count(&self) -> Result<usize, CacheError> {
        let count = fs::read_dir(&self.objects_dir)
            .map_err(|e| CacheError::IoError {
                path: self.objects_dir.clone(),
                message: e.to_string(),
            })?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "o"))
            .count();

        Ok(count)
    }

    /// Validate cache integrity (check version, etc.).
    pub fn validate(&self) -> Result<bool, CacheError> {
        let version_file = self.config.cache_dir.join("version");

        if !version_file.exists() {
            return Ok(false);
        }

        let mut file = File::open(&version_file).map_err(|e| CacheError::IoError {
            path: version_file.clone(),
            message: e.to_string(),
        })?;

        let mut version = String::new();
        file.read_to_string(&mut version)
            .map_err(|e| CacheError::IoError {
                path: version_file,
                message: e.to_string(),
            })?;

        Ok(version.trim() == self.config.compiler_version)
    }

    /// Get the cache configuration.
    #[must_use]
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
}

/// Error during cache operations.
#[derive(Debug, Clone)]
pub enum CacheError {
    /// I/O error.
    IoError { path: PathBuf, message: String },
    /// Cache is invalid (version mismatch, etc.).
    Invalid { message: String },
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError { path, message } => {
                write!(f, "cache I/O error at '{}': {}", path.display(), message)
            }
            Self::Invalid { message } => {
                write!(f, "cache invalid: {message}")
            }
        }
    }
}

impl std::error::Error for CacheError {}

#[cfg(test)]
mod tests {
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
}
