//! Source File Hashing
//!
//! Fast content-based hashing for incremental compilation.
//! Uses a combination of file metadata (size, mtime) for quick checks
//! and content hash for accurate change detection.

use rustc_hash::FxHashMap;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read};
use std::ops::BitXor;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A content hash representing the state of a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash(u64);

impl ContentHash {
    /// Create a new content hash from a u64 value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Get the underlying hash value.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.0
    }

    /// Format as a hex string.
    #[must_use]
    pub fn to_hex(&self) -> String {
        format!("{:016x}", self.0)
    }

    /// Parse from a hex string.
    pub fn from_hex(s: &str) -> Option<Self> {
        u64::from_str_radix(s, 16).ok().map(Self)
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

/// Metadata about a source file for quick change detection.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// Size of the file in bytes.
    pub size: u64,
    /// Last modification time.
    pub mtime: SystemTime,
    /// Content hash (computed lazily).
    pub content_hash: ContentHash,
}

impl FileMetadata {
    /// Check if the file might have changed based on metadata.
    ///
    /// Returns true if size or mtime differs, indicating a possible change.
    /// A false result means the file definitely changed.
    pub fn might_be_unchanged(&self, other: &Self) -> bool {
        self.size == other.size && self.mtime == other.mtime
    }
}

/// Source file hasher for incremental compilation.
#[derive(Debug)]
pub struct SourceHasher {
    /// Cache of file hashes keyed by path.
    cache: FxHashMap<PathBuf, FileMetadata>,
    /// Whether to normalize content before hashing (ignore whitespace changes).
    normalize: bool,
}

impl Default for SourceHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceHasher {
    /// Create a new source hasher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: FxHashMap::default(),
            normalize: false,
        }
    }

    /// Enable content normalization (experimental).
    ///
    /// When enabled, whitespace-only changes don't trigger recompilation.
    #[must_use]
    pub fn with_normalization(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Hash a source file's content.
    ///
    /// Uses a fast hash algorithm (`FxHash` via std Hasher) for speed.
    pub fn hash_file(&mut self, path: &Path) -> Result<ContentHash, HashError> {
        // Check cache first (metadata-based quick check)
        if let Some(cached) = self.cache.get(path) {
            if let Ok(meta) = fs::metadata(path) {
                let current_size = meta.len();
                let current_mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

                if cached.size == current_size && cached.mtime == current_mtime {
                    return Ok(cached.content_hash);
                }
            }
        }

        // Compute fresh hash
        let hash = self.compute_hash(path)?;

        // Update cache
        if let Ok(meta) = fs::metadata(path) {
            self.cache.insert(
                path.to_path_buf(),
                FileMetadata {
                    size: meta.len(),
                    mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    content_hash: hash,
                },
            );
        }

        Ok(hash)
    }

    /// Hash multiple files and combine into a single hash.
    ///
    /// Useful for computing a combined hash of all dependencies.
    pub fn hash_files(&mut self, paths: &[PathBuf]) -> Result<ContentHash, HashError> {
        let mut combined = FxHasher::default();

        // Sort paths for deterministic ordering
        let mut sorted_paths = paths.to_vec();
        sorted_paths.sort();

        for path in &sorted_paths {
            let hash = self.hash_file(path)?;
            hash.0.hash(&mut combined);
        }

        Ok(ContentHash(combined.finish()))
    }

    /// Compute the content hash of a file.
    fn compute_hash(&self, path: &Path) -> Result<ContentHash, HashError> {
        let file = File::open(path).map_err(|e| HashError::IoError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        let mut hasher = FxHasher::default();

        if self.normalize {
            Self::hash_normalized(file, &mut hasher)?;
        } else {
            Self::hash_raw(file, &mut hasher)?;
        }

        Ok(ContentHash(hasher.finish()))
    }

    /// Hash file content directly.
    fn hash_raw(mut file: File, state: &mut FxHasher) -> Result<(), HashError> {
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).map_err(|e| HashError::IoError {
                path: PathBuf::new(),
                message: e.to_string(),
            })?;
            if n == 0 {
                break;
            }
            buffer[..n].hash(state);
        }
        Ok(())
    }

    /// Hash normalized content (ignoring whitespace changes).
    fn hash_normalized(file: File, state: &mut FxHasher) -> Result<(), HashError> {
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.map_err(|e| HashError::IoError {
                path: PathBuf::new(),
                message: e.to_string(),
            })?;

            // Trim trailing whitespace and normalize line endings
            let trimmed = line.trim_end();
            trimmed.hash(state);
            '\n'.hash(state);
        }

        Ok(())
    }

    /// Clear the hash cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cached metadata for a file.
    #[must_use]
    pub fn get_cached(&self, path: &Path) -> Option<&FileMetadata> {
        self.cache.get(path)
    }

    /// Check if a file has changed since it was last hashed.
    pub fn has_changed(&mut self, path: &Path) -> Result<bool, HashError> {
        let old_hash = self.cache.get(path).map(|m| m.content_hash);
        let new_hash = self.hash_file(path)?;

        Ok(old_hash != Some(new_hash))
    }
}

/// Error during hashing.
#[derive(Debug, Clone)]
pub enum HashError {
    /// I/O error reading file.
    IoError { path: PathBuf, message: String },
    /// File not found.
    NotFound { path: PathBuf },
}

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError { path, message } => {
                write!(f, "failed to read '{}': {}", path.display(), message)
            }
            Self::NotFound { path } => {
                write!(f, "file not found: '{}'", path.display())
            }
        }
    }
}

impl std::error::Error for HashError {}

/// A fast, non-cryptographic hasher based on `FxHash`.
///
/// This is the same algorithm used by rustc for incremental compilation.
/// It's much faster than SHA-256 while still having good distribution.
#[derive(Default)]
struct FxHasher {
    hash: u64,
}

impl FxHasher {
    const K: u64 = 0x517c_c1b7_2722_0a95;
}

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash = self
                .hash
                .rotate_left(5)
                .bitxor(u64::from(*byte))
                .wrapping_mul(Self::K);
        }
    }

    fn finish(&self) -> u64 {
        self.hash
    }
}

/// Hash a string directly (for testing and simple use cases).
#[must_use]
pub fn hash_string(s: &str) -> ContentHash {
    let mut hasher = FxHasher::default();
    s.hash(&mut hasher);
    ContentHash(hasher.finish())
}

/// Hash raw bytes directly.
///
/// Used for hashing serialized data (e.g., bincode-encoded ARC IR).
#[must_use]
pub fn hash_bytes(data: &[u8]) -> ContentHash {
    let mut hasher = FxHasher::default();
    data.hash(&mut hasher);
    ContentHash(hasher.finish())
}

/// Combine multiple hashes into one.
#[must_use]
pub fn combine_hashes(hash_list: &[ContentHash]) -> ContentHash {
    let mut state = FxHasher::default();
    for hash in hash_list {
        hash.0.hash(&mut state);
    }
    ContentHash(state.finish())
}

#[cfg(test)]
mod tests {
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
        let path = create_temp_file("@main () -> void = print(msg: \"hello\")");
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
}
