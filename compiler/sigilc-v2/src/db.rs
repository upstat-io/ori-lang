//! Salsa database for incremental compilation.
//!
//! This module defines the core database trait and queries for
//! incremental compilation using Salsa.

use crate::intern::{StringInterner, TypeInterner};
use std::path::PathBuf;
use std::sync::Arc;

/// Durability levels for query caching.
///
/// Higher durability means the data changes less frequently,
/// allowing Salsa to skip more validation.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum Durability {
    /// User code being edited - check every revision
    #[default]
    Low,
    /// Project config - check occasionally
    Medium,
    /// Standard library - rarely changes
    High,
}

/// Main compiler database trait.
///
/// This is the central interface for all compiler queries.
/// Implementations provide access to interners and caches.
pub trait Db: Send + Sync {
    /// Access the string interner.
    fn interner(&self) -> &StringInterner;

    /// Access the type interner.
    fn type_interner(&self) -> &TypeInterner;

    /// Get or create a source file.
    fn get_source(&self, path: &PathBuf) -> Option<Arc<SourceFile>>;

    /// Set source file content.
    fn set_source(&self, path: PathBuf, content: String, durability: Durability);
}

/// Source file content with metadata.
#[derive(Clone, Debug)]
pub struct SourceFile {
    /// Absolute path to the file.
    pub path: PathBuf,
    /// Source text content.
    pub content: String,
    /// Durability level.
    pub durability: Durability,
}

/// Concrete implementation of the compiler database.
pub struct CompilerDb {
    /// String interner.
    interner: StringInterner,
    /// Type interner.
    type_interner: TypeInterner,
    /// Source files by path.
    sources: parking_lot::RwLock<rustc_hash::FxHashMap<PathBuf, Arc<SourceFile>>>,
}

impl CompilerDb {
    /// Create a new compiler database.
    pub fn new() -> Self {
        CompilerDb {
            interner: StringInterner::new(),
            type_interner: TypeInterner::new(),
            sources: parking_lot::RwLock::new(rustc_hash::FxHashMap::default()),
        }
    }

    /// Load a source file from disk.
    pub fn load_file(&self, path: PathBuf, durability: Durability) -> std::io::Result<()> {
        let content = std::fs::read_to_string(&path)?;
        self.set_source(path, content, durability);
        Ok(())
    }

    /// Get all loaded source files.
    pub fn sources(&self) -> Vec<Arc<SourceFile>> {
        self.sources.read().values().cloned().collect()
    }
}

impl Default for CompilerDb {
    fn default() -> Self {
        Self::new()
    }
}

impl Db for CompilerDb {
    fn interner(&self) -> &StringInterner {
        &self.interner
    }

    fn type_interner(&self) -> &TypeInterner {
        &self.type_interner
    }

    fn get_source(&self, path: &PathBuf) -> Option<Arc<SourceFile>> {
        self.sources.read().get(path).cloned()
    }

    fn set_source(&self, path: PathBuf, content: String, durability: Durability) {
        let file = Arc::new(SourceFile {
            path: path.clone(),
            content,
            durability,
        });
        self.sources.write().insert(path, file);
    }
}

// Note: Full Salsa integration will be added in Phase 1 Week 3-4.
// This is a simplified database that demonstrates the interface.
// The actual Salsa-based implementation will add:
// - #[salsa::input] for SourceFile
// - #[salsa::tracked] for parsed_module, typed_function, etc.
// - Automatic incremental recomputation
// - Early cutoff for unchanged outputs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_db_basic() {
        let db = CompilerDb::new();

        // Test string interning
        let name = db.interner().intern("test");
        assert_eq!(db.interner().lookup(name), "test");

        // Test type interning
        use crate::intern::{TypeId, TypeKind};
        assert_eq!(db.type_interner().intern(TypeKind::Int), TypeId::INT);
    }

    #[test]
    fn test_source_files() {
        let db = CompilerDb::new();

        db.set_source(
            PathBuf::from("/test/file.si"),
            "let x = 42".to_string(),
            Durability::Low,
        );

        let source = db.get_source(&PathBuf::from("/test/file.si")).unwrap();
        assert_eq!(source.content, "let x = 42");
    }
}
