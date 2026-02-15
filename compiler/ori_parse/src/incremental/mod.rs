//! Incremental Parsing Support
//!
//! This module provides infrastructure for incremental parsing, allowing the parser
//! to reuse unchanged AST subtrees when a document is edited.
//!
//! # Architecture
//!
//! The incremental parsing system works in phases:
//!
//! 1. **Declaration Collection** - Gather all top-level declarations with their spans
//! 2. **Cursor Navigation** - Find declarations that can be reused
//! 3. **Deep Copy** - Copy reusable declarations with span adjustment
//!
//! # Key Types
//!
//! - [`DeclKind`] - Categories of top-level declarations
//! - [`DeclRef`] - Reference to a declaration with its span
//! - [`SyntaxCursor`] - Navigator for finding reusable declarations
//! - [`IncrementalState`] - Session state tracking reuse statistics

mod copier;
mod cursor;
mod decl;

pub use copier::AstCopier;
pub use cursor::{CursorStats, SyntaxCursor};
pub use decl::{collect_declarations, DeclKind, DeclRef};

/// Statistics for incremental parsing.
#[derive(Clone, Debug, Default)]
pub struct IncrementalStats {
    /// Number of declarations reused from the old tree.
    pub reused_count: usize,
    /// Number of declarations that were reparsed.
    pub reparsed_count: usize,
}

impl IncrementalStats {
    /// Calculate reuse rate as a percentage.
    #[allow(
        clippy::cast_precision_loss,
        reason = "counts won't approach 2^52; precision loss irrelevant for display"
    )]
    pub fn reuse_rate(&self) -> f64 {
        let total = self.reused_count + self.reparsed_count;
        if total == 0 {
            0.0
        } else {
            (self.reused_count as f64 / total as f64) * 100.0
        }
    }
}

/// State for an incremental parsing session.
pub struct IncrementalState<'old> {
    pub cursor: SyntaxCursor<'old>,
    pub stats: IncrementalStats,
}

impl<'old> IncrementalState<'old> {
    /// Create a new incremental parsing state.
    pub fn new(cursor: SyntaxCursor<'old>) -> Self {
        IncrementalState {
            cursor,
            stats: IncrementalStats::default(),
        }
    }
}

#[cfg(test)]
mod tests;
