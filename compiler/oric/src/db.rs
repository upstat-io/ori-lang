//! Salsa Database - THE FOUNDATION
//!
//! This file is written FIRST, not last.
//! Everything else is built on top of this.
//!
//! # Architecture Notes
//!
//! ## File Caching (`RwLock`)
//!
//! The `file_cache` uses `parking_lot::RwLock` for efficient concurrent access
//! to the path→SourceFile deduplication map. This ensures we don't create
//! duplicate `SourceFile` inputs for the same path. The `SourceFile` values
//! themselves ARE tracked by Salsa - the cache is just an index to prevent
//! duplicates, not a substitute for Salsa tracking.
//!
//! ## Event Logging (Test-Only)
//!
//! The `logs` field uses `parking_lot::Mutex` for efficient test-time logging
//! of Salsa events. This is purely for debugging/testing and doesn't affect
//! Salsa's incremental tracking.

// Arc is required for Salsa database Clone
#![expect(
    clippy::disallowed_types,
    reason = "Arc required for Salsa database Clone"
)]

use crate::input::SourceFile;
use crate::ir::{SharedInterner, StringInterner};
use parking_lot::{Mutex, RwLock};
use salsa::Durability;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Session-scoped cache for type checking Pools.
///
/// The `Pool` cannot be a Salsa query result because it doesn't satisfy
/// `Clone + Eq + Hash`. This cache stores Pools as a side-channel: the
/// `typed()` Salsa query caches the Pool after type checking, and callers
/// that need both the `TypeCheckResult` and Pool retrieve it via `typed_pool()`.
///
/// Keyed by file path, since each `SourceFile` has a unique canonical path.
/// Invalidation is implicit: when `typed()` re-executes (source changed),
/// it overwrites the cached Pool with the new one.
#[derive(Clone, Default)]
pub struct PoolCache(Arc<RwLock<HashMap<PathBuf, Arc<ori_types::Pool>>>>);

/// Session-scoped cache for canonicalized module results.
///
/// `SharedCanonResult` doesn't satisfy Salsa's `Eq + Hash` requirements.
/// This cache avoids re-type-checking and re-canonicalizing the same module
/// across Evaluator instances (e.g., the prelude is canonicalized once per
/// session, not once per file in the test runner).
///
/// Keyed by `PathBuf` (same as `PoolCache`) for consistent key types across
/// all three side-caches.
#[derive(Clone, Default)]
pub struct CanonCache(Arc<RwLock<HashMap<PathBuf, ori_ir::canon::SharedCanonResult>>>);

/// Session-scoped cache for resolved imports.
///
/// `ResolvedImports` is assembled by walking prelude candidates, resolving
/// `use` statements, and building `ImportedFunctionRef` lists. This cache
/// avoids repeating that work when multiple consumers (type checker, evaluator,
/// LLVM backend) need imports for the same file within a single session.
///
/// Keyed by `PathBuf` (same as `PoolCache` and `CanonCache`) for consistent
/// key types across all three side-caches. Invalidation is implicit: each CLI
/// invocation creates a fresh `CompilerDb` with an empty cache. For future
/// watch-mode scenarios, the cache should be cleared when `parsed()` is
/// invalidated.
#[derive(Clone, Default)]
pub struct ImportsCache(
    Arc<RwLock<HashMap<PathBuf, std::sync::Arc<crate::imports::ResolvedImports>>>>,
);

impl PoolCache {
    /// Store a Pool for the given file path.
    pub fn store(&self, path: PathBuf, pool: ori_types::Pool) {
        self.0.write().insert(path, Arc::new(pool));
    }

    /// Retrieve the cached Pool for the given file path.
    pub fn get(&self, path: &Path) -> Option<Arc<ori_types::Pool>> {
        self.0.read().get(path).cloned()
    }
}

impl CanonCache {
    /// Store a canonicalized module result for the given module path.
    pub fn store(&self, module_path: PathBuf, canon: ori_ir::canon::SharedCanonResult) {
        self.0.write().insert(module_path, canon);
    }

    /// Retrieve the cached canon result for the given module path.
    pub fn get(&self, module_path: &Path) -> Option<ori_ir::canon::SharedCanonResult> {
        self.0.read().get(module_path).cloned()
    }

    /// Remove the cached canon result for the given module path.
    ///
    /// Called when Salsa re-executes `typed()` for a file, indicating
    /// that the source has changed and the cached result is stale.
    pub fn invalidate(&self, module_path: &Path) {
        self.0.write().remove(module_path);
    }
}

impl ImportsCache {
    /// Store resolved imports for the given file path.
    pub fn store(
        &self,
        file_path: PathBuf,
        imports: std::sync::Arc<crate::imports::ResolvedImports>,
    ) {
        self.0.write().insert(file_path, imports);
    }

    /// Retrieve cached resolved imports for the given file path.
    pub fn get(&self, file_path: &Path) -> Option<std::sync::Arc<crate::imports::ResolvedImports>> {
        self.0.read().get(file_path).cloned()
    }

    /// Remove the cached imports for the given file path.
    ///
    /// Called when Salsa re-executes `typed()` for a file, indicating
    /// that the source has changed and the cached result is stale.
    pub fn invalidate(&self, file_path: &Path) {
        self.0.write().remove(file_path);
    }
}

/// Main database trait that extends Salsa's Database.
///
/// All code that needs database access should use `&dyn Db`.
/// This gives access to both Salsa queries and the string interner.
#[salsa::db]
pub trait Db: salsa::Database {
    /// Get the string interner for interning identifiers and strings.
    fn interner(&self) -> &StringInterner;

    /// Load a source file by path, creating a `SourceFile` input if needed.
    ///
    /// This is the proper way to load imported files - it creates Salsa inputs
    /// so that changes to imported files are tracked and caches are invalidated.
    ///
    /// Returns None if the file cannot be read.
    fn load_file(&self, path: &Path) -> Option<SourceFile>;

    /// Access the session-scoped Pool cache.
    ///
    /// The Pool is stored as a side-channel during `typed()` execution because
    /// it can't satisfy Salsa's `Clone + Eq + Hash` requirements. Callers that
    /// need the Pool alongside the `TypeCheckResult` should use `typed_pool()`.
    fn pool_cache(&self) -> &PoolCache;

    /// Access the session-scoped canon result cache.
    ///
    /// Avoids re-canonicalizing the same module across Evaluator instances.
    fn canon_cache(&self) -> &CanonCache;

    /// Access the session-scoped imports cache.
    ///
    /// Avoids re-resolving imports when multiple consumers (type checker,
    /// evaluator, LLVM backend) need the same file's imports.
    fn imports_cache(&self) -> &ImportsCache;
}

/// Concrete implementation of the compiler database.
///
/// The #[`salsa::db`] macro generates much of the implementation.
/// This struct holds Salsa's storage plus any shared state.
///
/// MUST implement Clone for Salsa to work.
#[salsa::db]
#[derive(Clone)]
pub struct CompilerDb {
    /// Salsa's internal storage for all queries.
    storage: salsa::Storage<Self>,

    /// String interner for identifiers and string literals.
    /// Shared via Arc so Clone works and strings persist.
    interner: SharedInterner,

    /// Cache of loaded source files by path.
    ///
    /// Uses `parking_lot::RwLock` for efficient concurrent access. This is an
    /// index for deduplication only - the `SourceFile` values are Salsa inputs
    /// and are properly tracked. The cache prevents creating duplicate inputs
    /// for the same file path.
    ///
    /// Note: `parking_lot` types don't have poison errors, making error handling
    /// simpler and more robust than `std::sync` equivalents.
    file_cache: Arc<RwLock<HashMap<PathBuf, SourceFile>>>,

    /// Event logs for testing/debugging (optional).
    ///
    /// Uses `parking_lot::Mutex` for efficient locking. Wrapped in `Arc` so
    /// `Clone` works (required by Salsa). This is test-only and doesn't affect
    /// Salsa's incremental computation tracking.
    logs: Arc<Mutex<Option<Vec<String>>>>,

    /// Session-scoped Pool cache.
    ///
    /// Stores `Arc<Pool>` by file path, populated by the `typed()` Salsa query.
    /// See [`PoolCache`] for details.
    pool_cache: PoolCache,

    /// Session-scoped canon result cache.
    ///
    /// Stores `SharedCanonResult` by module path. Avoids re-canonicalizing
    /// the prelude and imported modules across Evaluator instances.
    canon_cache: CanonCache,

    /// Session-scoped imports cache.
    ///
    /// Stores `Arc<ResolvedImports>` by file path. Avoids re-resolving
    /// imports when multiple consumers need the same file's imports.
    imports_cache: ImportsCache,
}

impl Default for CompilerDb {
    fn default() -> Self {
        Self {
            storage: salsa::Storage::default(),
            interner: SharedInterner::new(),
            file_cache: Arc::default(),
            logs: Arc::default(),
            pool_cache: PoolCache::default(),
            canon_cache: CanonCache::default(),
            imports_cache: ImportsCache::default(),
        }
    }
}

impl CompilerDb {
    /// Create a new compiler database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a compiler database with an existing shared interner.
    ///
    /// This allows multiple databases to share the same interner, which is
    /// useful when compiling multiple files that need compatible `Name` values.
    pub fn with_interner(interner: SharedInterner) -> Self {
        Self {
            storage: salsa::Storage::default(),
            interner,
            file_cache: Arc::default(),
            logs: Arc::default(),
            pool_cache: PoolCache::default(),
            canon_cache: CanonCache::default(),
            imports_cache: ImportsCache::default(),
        }
    }

    /// Get the shared interner for use across databases.
    pub fn shared_interner(&self) -> SharedInterner {
        self.interner.clone()
    }

    /// Enable logging of Salsa events (for testing).
    #[cfg(test)]
    pub fn enable_logging(&self) {
        let mut logs = self.logs.lock();
        if logs.is_none() {
            *logs = Some(vec![]);
        }
    }

    /// Take the accumulated logs (for testing).
    #[cfg(test)]
    pub fn take_logs(&self) -> Vec<String> {
        let mut logs = self.logs.lock();
        if let Some(logs) = &mut *logs {
            std::mem::take(logs)
        } else {
            vec![]
        }
    }
}

/// Implement our Db trait for `CompilerDb`.
#[salsa::db]
impl Db for CompilerDb {
    fn interner(&self) -> &StringInterner {
        &self.interner
    }

    fn pool_cache(&self) -> &PoolCache {
        &self.pool_cache
    }

    fn canon_cache(&self) -> &CanonCache {
        &self.canon_cache
    }

    fn imports_cache(&self) -> &ImportsCache {
        &self.imports_cache
    }

    fn load_file(&self, path: &Path) -> Option<SourceFile> {
        // Canonicalize path for consistent caching
        let canonical = path.canonicalize().ok()?;

        // Check cache first (read lock for concurrent reads)
        {
            let cache = self.file_cache.read();
            if let Some(&file) = cache.get(&canonical) {
                return Some(file);
            }
        }

        // Read file and create SourceFile input.
        //
        // Stdlib/prelude files use Durability::HIGH because they don't change
        // between builds. This lets Salsa skip revalidating queries that depend
        // only on the prelude when the user edits their own files.
        let content = std::fs::read_to_string(&canonical).ok()?;
        let file = if is_stdlib_path(&canonical) {
            SourceFile::builder(canonical.clone(), content)
                .durability(Durability::HIGH)
                .new(self)
        } else {
            SourceFile::new(self, canonical.clone(), content)
        };

        // Cache it (write lock for insertion)
        {
            let mut cache = self.file_cache.write();
            cache.insert(canonical, file);
        }

        Some(file)
    }
}

/// Implement `salsa::Database` for `CompilerDb`.
///
/// The #[`salsa::db`] macro handles most of the implementation.
/// We just need to provide `salsa_event` for logging/debugging.
#[salsa::db]
impl salsa::Database for CompilerDb {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        let has_logs = self.logs.lock().is_some();
        let has_tracing = tracing::enabled!(tracing::Level::TRACE);

        // Skip event evaluation entirely if neither consumer is active
        if !has_logs && !has_tracing {
            return;
        }

        let event = event();

        // Bridge Salsa events to tracing
        match &event.kind {
            salsa::EventKind::WillExecute { .. } => {
                tracing::debug!(event = ?event.kind, "salsa: will execute");
            }
            salsa::EventKind::DidValidateMemoizedValue { .. } => {
                tracing::trace!(event = ?event.kind, "salsa: cache hit");
            }
            _ => {
                tracing::trace!(event = ?event.kind, "salsa event");
            }
        }

        // Keep in-memory log for tests
        if has_logs {
            if let salsa::EventKind::WillExecute { .. } = event.kind {
                if let Some(logs) = &mut *self.logs.lock() {
                    logs.push(format!("{event:?}"));
                }
            }
        }
    }
}

/// Check if a path belongs to the standard library.
///
/// Stdlib files (prelude, core modules) don't change between builds and are
/// marked with `Durability::HIGH` so Salsa can skip revalidating queries that
/// depend only on stable library code.
fn is_stdlib_path(path: &Path) -> bool {
    // Check for library/std/ directory in the path components
    let path_str = path.to_string_lossy();
    path_str.contains("/library/std/") || path_str.contains("\\library\\std\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_creation() {
        let _db = CompilerDb::new();
        // If this compiles and runs, Salsa is working
    }

    #[test]
    fn test_db_clone() {
        let db1 = CompilerDb::new();
        let _db2 = db1.clone();
        // Clone must work for Salsa
    }

    #[test]
    fn test_db_default() {
        let _db = CompilerDb::default();
    }

    #[test]
    fn test_is_stdlib_path() {
        // Unix-style paths
        assert!(is_stdlib_path(Path::new(
            "/home/user/ori/library/std/prelude.ori"
        )));
        assert!(is_stdlib_path(Path::new(
            "/home/user/ori/library/std/io.ori"
        )));

        // Windows-style paths
        assert!(is_stdlib_path(Path::new(
            "C:\\Users\\user\\ori\\library\\std\\prelude.ori"
        )));

        // User files are NOT stdlib
        assert!(!is_stdlib_path(Path::new("/home/user/project/main.ori")));
        assert!(!is_stdlib_path(Path::new("/home/user/project/src/lib.ori")));
    }
}
