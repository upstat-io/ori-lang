//! Salsa Database - THE FOUNDATION
//!
//! This file is written FIRST, not last.
//! Everything else is built on top of this.

use std::sync::{Arc, Mutex};
use crate::ir::{StringInterner, SharedInterner};

/// Main database trait that extends Salsa's Database.
///
/// All code that needs database access should use `&dyn Db`.
/// This gives access to both Salsa queries and the string interner.
#[salsa::db]
pub trait Db: salsa::Database {
    /// Get the string interner for interning identifiers and strings.
    fn interner(&self) -> &StringInterner;
}

/// Concrete implementation of the compiler database.
///
/// The #[salsa::db] macro generates much of the implementation.
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

    /// Event logs for testing/debugging (optional).
    /// Wrapped in Arc<Mutex> so Clone works.
    logs: Arc<Mutex<Option<Vec<String>>>>,
}

impl Default for CompilerDb {
    fn default() -> Self {
        Self {
            storage: Default::default(),
            interner: Arc::new(StringInterner::new()),
            logs: Default::default(),
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
    pub fn enable_logging(&self) {
        let mut logs = self.logs.lock().unwrap();
        if logs.is_none() {
            *logs = Some(vec![]);
        }
    }

    /// Take the accumulated logs (for testing).
    #[cfg(test)]
    pub fn take_logs(&self) -> Vec<String> {
        let mut logs = self.logs.lock().unwrap();
        if let Some(logs) = &mut *logs {
            std::mem::take(logs)
        } else {
            vec![]
        }
    }
}

/// Implement our Db trait for CompilerDb.
#[salsa::db]
impl Db for CompilerDb {
    fn interner(&self) -> &StringInterner {
        &self.interner
    }
}

/// Implement salsa::Database for CompilerDb.
///
/// The #[salsa::db] macro handles most of the implementation.
/// We just need to provide salsa_event for logging/debugging.
#[salsa::db]
impl salsa::Database for CompilerDb {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        // Log events if logging is enabled
        if let Some(logs) = &mut *self.logs.lock().unwrap() {
            let event = event();
            // Only log execution events (most interesting for debugging)
            if let salsa::EventKind::WillExecute { .. } = event.kind {
                logs.push(format!("{:?}", event));
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
        let _db: CompilerDb = Default::default();
    }
}
