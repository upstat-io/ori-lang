//! Code Fix Registry
//!
//! Manages registration and lookup of code fixes by error code.
//!
//! # Design Note
//!
//! Currently uses `Arc<dyn CodeFix>` for trait object storage. This is fine
//! for the current use case where zero production fixes exist. When built-in
//! fixes are implemented, consider evaluating enum dispatch for better
//! performance if profiling shows the vtable indirection is significant.

#![expect(
    clippy::disallowed_types,
    reason = "Box/Arc needed for trait object storage"
)]

use std::collections::HashMap;
use std::sync::Arc;

use crate::ErrorCode;

use super::{CodeAction, CodeFix, FixContext};

/// Registry for code fixes.
///
/// Fixes are registered for specific error codes. When a diagnostic is
/// encountered, the registry finds all applicable fixes.
pub struct FixRegistry {
    /// All registered fixes (stored once, referenced by error code).
    fixes: Vec<Arc<dyn CodeFix>>,
    /// Index from error code to fix indices.
    by_code: HashMap<ErrorCode, Vec<usize>>,
}

impl Default for FixRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FixRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        FixRegistry {
            fixes: Vec::new(),
            by_code: HashMap::new(),
        }
    }

    /// Register a code fix.
    ///
    /// The fix will be available for all error codes it declares.
    pub fn register<F: CodeFix + 'static>(&mut self, fix: F) {
        let fix = Arc::new(fix);
        let idx = self.fixes.len();

        // Iterate directly over slice, no allocation
        for &code in fix.error_codes() {
            self.by_code.entry(code).or_default().push(idx);
        }

        self.fixes.push(fix);
    }

    /// Get all code actions for a diagnostic.
    pub fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        let mut actions = Vec::new();

        if let Some(indices) = self.by_code.get(&ctx.diagnostic.code) {
            for &idx in indices {
                actions.extend(self.fixes[idx].get_fixes(ctx));
            }
        }

        actions
    }

    /// Check if any fixes are registered for the given code.
    pub fn has_fixes_for(&self, code: ErrorCode) -> bool {
        self.by_code.contains_key(&code)
    }

    /// Get the number of registered fixes.
    pub fn fix_count(&self) -> usize {
        self.fixes.len()
    }

    /// Get the number of error code -> fix mappings.
    pub fn mapping_count(&self) -> usize {
        self.by_code.values().map(std::vec::Vec::len).sum()
    }
}

impl std::fmt::Debug for FixRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FixRegistry")
            .field("fix_count", &self.fix_count())
            .field("codes", &self.by_code.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
