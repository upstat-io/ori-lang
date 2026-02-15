//! ARC IR Cache
//!
//! Caches serialized ARC IR (the output of borrow inference, RC insertion,
//! elimination, and constructor reuse) to avoid re-running ARC analysis
//! for unchanged modules.
//!
//! # Cache Granularity (0.1-alpha)
//!
//! Per-module: all functions in a module are cached together, keyed by the
//! combined hash of all function hashes. If ANY function changes, the entire
//! module's ARC IR is re-analyzed. This is simpler than per-function caching
//! while still providing good benefit (unchanged modules skip ARC analysis
//! entirely).
//!
//! # Cache Directory Structure
//!
//! ```text
//! build/cache/functions/arc_ir/
//! ├── <content_hash>.bin    # Bincode-encoded Vec<ArcFunction>
//! └── ...
//! ```

use std::path::{Path, PathBuf};

use ori_arc::ArcFunction;

use super::hash::ContentHash;

/// Cache key for ARC IR, based on the module's function content hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArcIrCacheKey {
    /// Combined hash of all function hashes in the module.
    pub function_hash: ContentHash,
}

/// Cached ARC IR data (serialized `Vec<ArcFunction>`).
#[derive(Debug, Clone)]
pub struct CachedArcIr {
    /// Bincode-encoded data.
    pub data: Vec<u8>,
    /// Hash of the data for integrity verification.
    pub hash: ContentHash,
}

impl CachedArcIr {
    /// Serialize a list of ARC functions into a cached representation.
    pub fn from_arc_functions(funcs: &[ArcFunction]) -> Result<Self, String> {
        let data =
            bincode::serialize(funcs).map_err(|e| format!("failed to serialize ARC IR: {e}"))?;
        let hash = super::hash::hash_bytes(&data);
        Ok(Self { data, hash })
    }

    /// Deserialize ARC functions from the cached data.
    pub fn to_arc_functions(&self) -> Result<Vec<ArcFunction>, String> {
        bincode::deserialize(&self.data).map_err(|e| format!("failed to deserialize ARC IR: {e}"))
    }
}

/// Cache for per-module ARC IR analysis results.
///
/// Stores serialized `Vec<ArcFunction>` keyed by the module's combined
/// function content hash. If the hash matches, ARC analysis can be
/// skipped entirely.
pub struct ArcIrCache {
    /// Directory for cache files.
    cache_dir: PathBuf,
}

impl ArcIrCache {
    /// Create a new ARC IR cache at the given directory.
    ///
    /// Creates the directory structure if it doesn't exist.
    pub fn new(cache_dir: &Path) -> Result<Self, String> {
        let arc_cache_dir = cache_dir.join("functions").join("arc_ir");
        std::fs::create_dir_all(&arc_cache_dir)
            .map_err(|e| format!("failed to create ARC IR cache directory: {e}"))?;
        Ok(Self {
            cache_dir: arc_cache_dir,
        })
    }

    /// Get cached ARC IR for a module.
    ///
    /// Returns `None` on cache miss (file not found or corrupt).
    pub fn get(&self, key: &ArcIrCacheKey) -> Option<CachedArcIr> {
        let path = self.cache_path(key);
        let data = std::fs::read(&path).ok()?;
        let hash = super::hash::hash_bytes(&data);
        Some(CachedArcIr { data, hash })
    }

    /// Store ARC IR in the cache.
    pub fn put(&self, key: &ArcIrCacheKey, cached: &CachedArcIr) -> Result<(), String> {
        let path = self.cache_path(key);
        std::fs::write(&path, &cached.data)
            .map_err(|e| format!("failed to write ARC IR cache: {e}"))
    }

    /// Check if a cache entry exists for the given key.
    #[must_use]
    pub fn has(&self, key: &ArcIrCacheKey) -> bool {
        self.cache_path(key).exists()
    }

    /// Compute the file path for a cache key.
    fn cache_path(&self, key: &ArcIrCacheKey) -> PathBuf {
        self.cache_dir.join(format!("{}.bin", key.function_hash))
    }

    /// Clear the entire cache.
    pub fn clear(&self) -> Result<(), String> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)
                .map_err(|e| format!("failed to clear ARC IR cache: {e}"))?;
            std::fs::create_dir_all(&self.cache_dir)
                .map_err(|e| format!("failed to recreate ARC IR cache directory: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
