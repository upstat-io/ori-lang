//! Salsa Database - THE FOUNDATION
//!
//! This file is written FIRST, not last.
//! Everything else is built on top of this.

// Arc and Mutex are required for Salsa database and thread-safe logging
#![expect(
    clippy::disallowed_types,
    reason = "Arc/Mutex required for Salsa database"
)]

use crate::input::SourceFile;
use crate::ir::{SharedInterner, StringInterner};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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
    /// This ensures imported files become proper Salsa inputs.
    file_cache: Arc<Mutex<HashMap<PathBuf, SourceFile>>>,

    /// Event logs for testing/debugging (optional).
    /// Wrapped in Arc<Mutex> so Clone works.
    logs: Arc<Mutex<Option<Vec<String>>>>,
}

impl Default for CompilerDb {
    fn default() -> Self {
        Self {
            storage: salsa::Storage::default(),
            interner: SharedInterner::new(),
            file_cache: Arc::default(),
            logs: Arc::default(),
        }
    }
}

impl CompilerDb {
    /// Create a new compiler database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable logging of Salsa events (for testing).
    #[cfg(test)]
    #[expect(clippy::unwrap_used, reason = "Test-only method uses unwrap")]
    pub fn enable_logging(&self) {
        let mut logs = self.logs.lock().unwrap();
        if logs.is_none() {
            *logs = Some(vec![]);
        }
    }

    /// Take the accumulated logs (for testing).
    #[cfg(test)]
    #[expect(clippy::unwrap_used, reason = "Test-only method uses unwrap")]
    pub fn take_logs(&self) -> Vec<String> {
        let mut logs = self.logs.lock().unwrap();
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

    fn load_file(&self, path: &Path) -> Option<SourceFile> {
        // Canonicalize path for consistent caching
        let canonical = path.canonicalize().ok()?;

        // Check cache first
        {
            let cache = self
                .file_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(&file) = cache.get(&canonical) {
                return Some(file);
            }
        }

        // Read file and create SourceFile input
        let content = std::fs::read_to_string(&canonical).ok()?;
        let file = SourceFile::new(self, canonical.clone(), content);

        // Cache it
        {
            let mut cache = self
                .file_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        // Log events if logging is enabled
        if let Some(logs) = &mut *self
            .logs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            let event = event();
            // Only log execution events (most interesting for debugging)
            if let salsa::EventKind::WillExecute { .. } = event.kind {
                logs.push(format!("{event:?}"));
            }
        }
    }
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
}
