//! Shared expression arena wrapper.

// Arc is needed for SharedArena - the implementation of shared arena references
#![expect(
    clippy::disallowed_types,
    reason = "Arc is the implementation of SharedArena"
)]

use std::fmt;
use std::sync::Arc;

use super::ExprArena;

/// Shared expression arena wrapper for cross-module function references.
///
/// This newtype enforces that all arena sharing goes through this type,
/// preventing accidental direct `Arc<ExprArena>` usage.
///
/// # Purpose
/// When importing functions from other modules, the function's body expression
/// references expressions in the imported module's arena. `SharedArena` allows
/// the imported function to carry its arena reference for correct evaluation.
///
/// # Thread Safety
/// Uses `Arc` internally for thread-safe reference counting.
///
/// # Usage
///
/// `ParseOutput.arena` is already a `SharedArena`, so cloning is O(1):
/// ```text
/// let arena = parse_result.arena.clone(); // Arc::clone, not deep copy
/// let func = FunctionValue::new(params, captures, arena);
/// ```
#[derive(Clone)]
pub struct SharedArena(Arc<ExprArena>);

impl SharedArena {
    /// Create a new shared arena from an `ExprArena`.
    pub fn new(arena: ExprArena) -> Self {
        SharedArena(Arc::new(arena))
    }
}

impl std::ops::Deref for SharedArena {
    type Target = ExprArena;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for SharedArena {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

impl Eq for SharedArena {}

impl std::hash::Hash for SharedArena {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for SharedArena {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SharedArena({:?})", &*self.0)
    }
}
