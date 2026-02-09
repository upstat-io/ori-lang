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
mod tests {
    use super::*;
    use ori_arc::{
        ArcBlock, ArcBlockId, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId, LitValue,
        Ownership,
    };
    use ori_ir::Name;
    use ori_types::Idx;

    fn sample_arc_function() -> ArcFunction {
        ArcFunction {
            name: Name::from_raw(1),
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Owned,
            }],
            return_type: Idx::INT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(1),
                },
            }],
            entry: ArcBlockId::new(0),
            var_types: vec![Idx::INT, Idx::INT],
            spans: vec![vec![None]],
        }
    }

    #[test]
    fn test_cached_arc_ir_roundtrip() {
        let funcs = vec![sample_arc_function()];

        let cached = CachedArcIr::from_arc_functions(&funcs)
            .unwrap_or_else(|e| panic!("serialize failed: {e}"));

        let restored = cached
            .to_arc_functions()
            .unwrap_or_else(|e| panic!("deserialize failed: {e}"));

        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].name, funcs[0].name);
        assert_eq!(restored[0].blocks, funcs[0].blocks);
        // Spans are skipped in serialization
        assert!(restored[0].spans.is_empty());
    }

    #[test]
    fn test_arc_cache_put_get() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
        let cache =
            ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

        let key = ArcIrCacheKey {
            function_hash: ContentHash::new(12345),
        };

        // Cache miss
        assert!(!cache.has(&key));
        assert!(cache.get(&key).is_none());

        // Put
        let cached = CachedArcIr::from_arc_functions(&[sample_arc_function()])
            .unwrap_or_else(|e| panic!("serialize failed: {e}"));
        cache
            .put(&key, &cached)
            .unwrap_or_else(|e| panic!("put failed: {e}"));

        // Cache hit
        assert!(cache.has(&key));
        let retrieved = cache
            .get(&key)
            .unwrap_or_else(|| panic!("cache should contain entry"));
        let funcs = retrieved
            .to_arc_functions()
            .unwrap_or_else(|e| panic!("deserialize failed: {e}"));
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, Name::from_raw(1));
    }

    #[test]
    fn test_arc_cache_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
        let cache =
            ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

        let key = ArcIrCacheKey {
            function_hash: ContentHash::new(99999),
        };

        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_arc_cache_clear() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
        let cache =
            ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

        let key = ArcIrCacheKey {
            function_hash: ContentHash::new(42),
        };
        let cached = CachedArcIr::from_arc_functions(&[sample_arc_function()])
            .unwrap_or_else(|e| panic!("serialize failed: {e}"));
        cache
            .put(&key, &cached)
            .unwrap_or_else(|e| panic!("put failed: {e}"));
        assert!(cache.has(&key));

        cache
            .clear()
            .unwrap_or_else(|e| panic!("clear failed: {e}"));
        assert!(!cache.has(&key));
    }

    #[test]
    fn test_different_hash_different_entry() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
        let cache =
            ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

        let key1 = ArcIrCacheKey {
            function_hash: ContentHash::new(100),
        };
        let key2 = ArcIrCacheKey {
            function_hash: ContentHash::new(200),
        };

        let cached = CachedArcIr::from_arc_functions(&[sample_arc_function()])
            .unwrap_or_else(|e| panic!("serialize failed: {e}"));
        cache
            .put(&key1, &cached)
            .unwrap_or_else(|e| panic!("put failed: {e}"));

        // key1 hits, key2 misses
        assert!(cache.has(&key1));
        assert!(!cache.has(&key2));
    }
}
