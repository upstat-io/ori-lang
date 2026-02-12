//! Parser snapshots for speculative parsing.
//!
//! Snapshots enable the parser to:
//! - Try a parse speculatively
//! - Examine the result
//! - Decide whether to keep or discard it
//!
//! # When to Use Each Approach
//!
//! ## Simple Lookahead (prefer when possible)
//!
//! Use direct token checks when:
//! - You only need to peek 1-2 tokens ahead
//! - The decision is based on token kinds, not parse success
//! - No complex context switching is needed
//!
//! ```ignore
//! // Good: Simple 1-2 token lookahead
//! fn is_typed_lambda_params(&self) -> bool {
//!     self.check_ident() && self.next_is_colon()
//! }
//! ```
//!
//! ## `look_ahead()` Method
//!
//! Use when:
//! - You need to peek more than 2 tokens ahead
//! - The lookahead involves skipping newlines or complex token patterns
//! - You want to run a predicate that consumes tokens but shouldn't affect state
//!
//! ```ignore
//! // Good: Complex multi-token lookahead
//! let is_with_capability = self.look_ahead(|p| {
//!     p.cursor.check(&TokenKind::With)
//!         && p.cursor.advance()
//!         && p.cursor.check_ident()
//!         && p.cursor.advance()
//!         && p.cursor.check(&TokenKind::Eq)
//! });
//! ```
//!
//! ## `try_parse()` Method
//!
//! Use when:
//! - You need to attempt a full parse and check if it succeeds
//! - The decision requires evaluating parse success, not just tokens
//! - You want automatic restoration on failure
//!
//! ```ignore
//! // Good: Try full parse, fall back on failure
//! if let Some(ty) = self.try_parse(|p| p.parse_type()) {
//!     // Type parsed successfully
//! } else {
//!     // Fall back to expression parsing
//! }
//! ```
//!
//! ## `snapshot()` / `restore()` Methods
//!
//! Use when:
//! - You need manual control over when to restore
//! - You want to examine the parse result before deciding
//! - You need to handle multiple alternative restorations
//!
//! ```ignore
//! // Good: Manual snapshot for complex decision
//! let snapshot = self.snapshot();
//! match self.parse_complex_syntax() {
//!     Ok(result) if self.check(&TokenKind::SomeMarker) => {
//!         // Keep the parse - don't restore
//!         return Ok(result);
//!     }
//!     _ => {
//!         // Restore and try alternative
//!         self.restore(snapshot);
//!         return self.parse_alternative();
//!     }
//! }
//! ```
//!
//! # Design Notes
//!
//! Snapshots are lightweight (~10 bytes) and only capture:
//! - Cursor position
//! - Parse context flags
//!
//! **Arena state is NOT captured.** Speculative parsing should only examine
//! tokens, not allocate AST nodes. If you need to allocate during speculation,
//! ensure the allocations are acceptable even if you later restore.
//!
//! # Current Usage in Ori Parser
//!
//! The Ori parser currently uses simple lookahead predicates for most
//! disambiguation because they're sufficient and efficient:
//! - `is_typed_lambda_params()`: ident + colon check
//! - `is_with_capability_syntax()`: 3-token manual lookahead
//! - `allows_struct_lit()`: context flag check
//!
//! The snapshot infrastructure is available for future needs such as:
//! - IDE tooling (try parse, capture errors, restore)
//! - More complex disambiguation in language extensions
//! - Better error message generation via speculative parsing

use crate::context::ParseContext;

/// A lightweight snapshot of parser state for speculative parsing.
///
/// Captures minimal state needed to restore the parser after trying
/// a speculative parse. Does not capture arena state—only use for
/// speculative parsing that doesn't allocate.
///
/// # Size
///
/// - `cursor_pos`: 8 bytes
/// - `context`: 2 bytes
/// - Total: 10 bytes (plus padding)
#[derive(Clone, Copy, Debug)]
pub struct ParserSnapshot {
    /// Position in the token stream.
    pub(crate) cursor_pos: usize,
    /// Parse context flags (`IN_LOOP`, `IN_TYPE`, etc.).
    pub(crate) context: ParseContext,
}

impl ParserSnapshot {
    /// Create a new snapshot with the given state.
    #[inline]
    pub(crate) fn new(cursor_pos: usize, context: ParseContext) -> Self {
        Self {
            cursor_pos,
            context,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_size() {
        // Verify snapshot is lightweight
        assert!(
            std::mem::size_of::<ParserSnapshot>() <= 24,
            "ParserSnapshot should be small (got {} bytes)",
            std::mem::size_of::<ParserSnapshot>()
        );
    }

    #[test]
    fn test_snapshot_creation() {
        let snapshot = ParserSnapshot::new(42, ParseContext::IN_LOOP);
        assert_eq!(snapshot.cursor_pos, 42);
        assert!(snapshot.context.in_loop());
    }

    #[test]
    fn test_snapshot_copy() {
        let snapshot1 = ParserSnapshot::new(10, ParseContext::IN_TYPE);
        let snapshot2 = snapshot1; // Copy
        assert_eq!(snapshot1.cursor_pos, snapshot2.cursor_pos);
        assert_eq!(snapshot1.context, snapshot2.context);
    }
}
