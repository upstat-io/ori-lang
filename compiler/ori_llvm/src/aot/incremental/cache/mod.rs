//! Artifact Cache for Incremental Compilation
//!
//! Caches compiled object files and other artifacts to avoid recompilation.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::hash::{combine_hashes, hash_string, ContentHash};

mod atomic;

use atomic::{publish_bytes_atomically, publish_generation_bytes, publish_generation_file};

const ENTRY_ENVELOPE_VERSION: &[u8] = b"ori-artifact-entry-v1\n";
const CACHE_KEY_HEX_LEN: usize = 16;
const SHA256_HEX_LEN: usize = 64;

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

fn object_file_stem(key: &CacheKey, object_id: &str) -> Result<String, CacheError> {
    if !is_lowercase_hex(object_id, SHA256_HEX_LEN) {
        return Err(CacheError::Invalid {
            message: "object ID must be a 64-character lowercase SHA-256 hexadecimal digest"
                .to_string(),
        });
    }

    Ok(format!("{}-{object_id}", key.to_filename()))
}

fn encode_entry_envelope(object_file: &str, manifest: &[u8]) -> Vec<u8> {
    let mut envelope = ENTRY_ENVELOPE_VERSION.to_vec();
    envelope.extend_from_slice(object_file.as_bytes());
    envelope.push(b'\n');
    envelope.extend_from_slice(manifest);
    envelope
}

fn decode_entry_envelope(envelope: &[u8]) -> Option<(&str, &[u8])> {
    let entry = envelope.strip_prefix(ENTRY_ENVELOPE_VERSION)?;
    let file_name_end = entry.iter().position(|byte| *byte == b'\n')?;
    let object_file = std::str::from_utf8(&entry[..file_name_end]).ok()?;
    if !is_safe_object_file_name(object_file) {
        return None;
    }

    Some((object_file, &entry[file_name_end + 1..]))
}

fn is_safe_object_file_name(file_name: &str) -> bool {
    if file_name.contains('/')
        || file_name.contains('\\')
        || Path::new(file_name)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(file_name)
    {
        return false;
    }

    let Some(stem) = file_name.strip_suffix(".o") else {
        return false;
    };
    let mut parts = stem.split('-');
    let Some(key_hash) = parts.next() else {
        return false;
    };
    let Some(object_id) = parts.next() else {
        return false;
    };
    let Some(process_id) = parts.next() else {
        return false;
    };
    let Some(generation) = parts.next() else {
        return false;
    };

    parts.next().is_none()
        && is_lowercase_hex(key_hash, CACHE_KEY_HEX_LEN)
        && is_lowercase_hex(object_id, SHA256_HEX_LEN)
        && is_ascii_decimal(process_id)
        && is_ascii_decimal(generation)
}

fn is_lowercase_hex(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_ascii_decimal(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
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

        // Publish the version marker atomically so concurrent cache openers
        // never expose a truncated marker.
        let version_file = config.cache_dir.join("version");
        publish_bytes_atomically(&version_file, config.compiler_version.as_bytes())?;

        Ok(Self {
            config,
            objects_dir,
            meta_dir,
        })
    }

    fn meta_path(&self, key: &CacheKey) -> PathBuf {
        self.meta_dir.join(format!("{}.json", key.to_filename()))
    }

    /// Return a cache entry only after the caller validates its manifest.
    ///
    /// The cache owns publication ordering and generation lookup. The caller
    /// owns the manifest schema and semantic request identity.
    pub fn get_verified(
        &self,
        key: &CacheKey,
        verify: impl FnOnce(&Path, &[u8]) -> bool,
    ) -> Option<PathBuf> {
        let envelope = fs::read(self.meta_path(key)).ok()?;
        let (object_file, manifest) = decode_entry_envelope(&envelope)?;
        let object = self.objects_dir.join(object_file);
        if object.is_file() && verify(&object, manifest) {
            Some(object)
        } else {
            None
        }
    }

    /// Publish object bytes and their caller-authored manifest as one entry.
    pub fn publish(
        &self,
        key: &CacheKey,
        object_id: &str,
        object_data: &[u8],
        manifest: &[u8],
    ) -> Result<(), CacheError> {
        let object_stem = object_file_stem(key, object_id)?;
        let object_file = publish_generation_bytes(&self.objects_dir, &object_stem, object_data)?;
        self.publish_manifest(key, &object_file, manifest)
    }

    /// Publish an object file and its caller-authored manifest as one entry.
    pub fn publish_file(
        &self,
        key: &CacheKey,
        object_id: &str,
        source: &Path,
        manifest: &[u8],
    ) -> Result<(), CacheError> {
        let object_stem = object_file_stem(key, object_id)?;
        let object_file = publish_generation_file(&self.objects_dir, &object_stem, source)?;
        self.publish_manifest(key, &object_file, manifest)
    }

    fn publish_manifest(
        &self,
        key: &CacheKey,
        object_file: &str,
        manifest: &[u8],
    ) -> Result<(), CacheError> {
        if !is_safe_object_file_name(object_file) {
            return Err(CacheError::Invalid {
                message: "generated object filename is not a safe cache-directory component"
                    .to_string(),
            });
        }

        let envelope = encode_entry_envelope(object_file, manifest);
        publish_bytes_atomically(&self.meta_path(key), &envelope)
    }

    /// Invalidate a cache entry while preserving its immutable object generation.
    ///
    /// A concurrent linker may still hold the object path returned by
    /// [`Self::get_verified`]. [`Self::clear`] reclaims retained generations.
    pub fn remove(&self, key: &CacheKey) -> Result<(), CacheError> {
        let meta_path = self.meta_path(key);
        match fs::remove_file(&meta_path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(CacheError::IoError {
                path: meta_path,
                message: error.to_string(),
            }),
        }
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

            let meta = entry.metadata().map_err(|e| CacheError::IoError {
                path: entry.path(),
                message: e.to_string(),
            })?;
            total += meta.len();
        }

        Ok(total)
    }

    /// Get the number of cached objects.
    pub fn count(&self) -> Result<usize, CacheError> {
        let count = fs::read_dir(&self.meta_dir)
            .map_err(|e| CacheError::IoError {
                path: self.meta_dir.clone(),
                message: e.to_string(),
            })?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
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
mod tests;
