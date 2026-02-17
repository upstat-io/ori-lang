//! Type checker warnings.
//!
//! Warnings indicate suspicious but valid code patterns. They do not
//! prevent compilation or evaluation — they are informational diagnostics.
//!
//! # Salsa Compatibility
//!
//! All types derive `Clone, Eq, PartialEq, Hash, Debug` for use in query results.

use ori_diagnostic::ErrorCode;
use ori_ir::Span;

/// A type checker warning.
///
/// Lighter than [`TypeCheckError`](super::TypeCheckError) — warnings carry
/// only the information needed for rendering, not full context/suggestions.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct TypeCheckWarning {
    /// Location in source code where the warning applies.
    pub span: Span,
    /// What kind of warning this is.
    pub kind: TypeCheckWarningKind,
}

/// The kind of type checker warning.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TypeCheckWarningKind {
    /// An infinite iterator is consumed by a method that will never terminate.
    ///
    /// Examples: `repeat(x).collect()`, `(0..).count()`, `iter.cycle().fold(...)`.
    /// Fix: add `.take(n)` before the consuming method.
    InfiniteIteratorConsumed {
        /// The consuming method name (e.g., "collect", "count", "fold").
        consumer: String,
        /// Description of the infinite source (e.g., "`repeat()`", "0..", "`cycle()`").
        source: String,
    },
}

impl TypeCheckWarning {
    /// Create an infinite-iterator-consumed warning.
    pub fn infinite_iterator_consumed(
        span: Span,
        consumer: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            span,
            kind: TypeCheckWarningKind::InfiniteIteratorConsumed {
                consumer: consumer.into(),
                source: source.into(),
            },
        }
    }

    /// Get the error code for this warning.
    pub fn code(&self) -> ErrorCode {
        match &self.kind {
            TypeCheckWarningKind::InfiniteIteratorConsumed { .. } => ErrorCode::W2001,
        }
    }
}
