//! Incremental Parsing Support
//!
//! Types for tracking text changes and determining which AST regions are affected.
//!
//! # Architecture
//!
//! This module provides the foundation for incremental parsing, following TypeScript's
//! proven two-phase approach:
//!
//! 1. **Text Change** - Represents an edit operation (insertion, deletion, replacement)
//! 2. **Change Marker** - Determines which source regions are affected by the change
//!
//! # LSP Integration
//!
//! `TextChange` maps directly to LSP's `TextDocumentContentChangeEvent`, making it
//! straightforward to integrate with language server implementations.

use crate::Span;

/// A single text edit, matching LSP's `TextDocumentContentChangeEvent`.
///
/// Represents a change to a text document where a region `[start, old_end)` in the
/// old text is replaced with `new_len` bytes of new text.
///
/// # Examples
///
/// ```
/// use ori_ir::incremental::TextChange;
///
/// // Insert "hello" at position 10
/// let insert = TextChange::insert(10, 5);
/// assert_eq!(insert.delta(), 5);
///
/// // Delete 3 characters starting at position 5
/// let delete = TextChange::delete(5, 3);
/// assert_eq!(delete.delta(), -3);
///
/// // Replace "foo" (3 chars) with "hello" (5 chars) at position 0
/// let replace = TextChange::replace(0, 3, 5);
/// assert_eq!(replace.delta(), 2);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TextChange {
    /// Start byte offset in old text.
    pub start: u32,
    /// End byte offset in old text (exclusive).
    pub old_end: u32,
    /// Length of replacement text in bytes.
    pub new_len: u32,
}

impl TextChange {
    /// Create a new text change.
    #[inline]
    pub const fn new(start: u32, old_end: u32, new_len: u32) -> Self {
        TextChange {
            start,
            old_end,
            new_len,
        }
    }

    /// Create an insertion (no characters removed).
    #[inline]
    pub const fn insert(at: u32, len: u32) -> Self {
        TextChange {
            start: at,
            old_end: at,
            new_len: len,
        }
    }

    /// Create a deletion (no characters inserted).
    #[inline]
    pub const fn delete(start: u32, len: u32) -> Self {
        TextChange {
            start,
            old_end: start + len,
            new_len: 0,
        }
    }

    /// Create a replacement.
    #[inline]
    pub const fn replace(start: u32, old_len: u32, new_len: u32) -> Self {
        TextChange {
            start,
            old_end: start + old_len,
            new_len,
        }
    }

    /// Net change in document length (positive = grew, negative = shrank).
    #[inline]
    pub fn delta(&self) -> i64 {
        i64::from(self.new_len) - i64::from(self.old_end - self.start)
    }

    /// Length of the removed region in the old text.
    #[inline]
    pub const fn old_len(&self) -> u32 {
        self.old_end - self.start
    }

    /// Check if this change intersects with a span in the old text.
    ///
    /// Two ranges intersect if they share any byte positions.
    #[inline]
    pub fn intersects(&self, span: Span) -> bool {
        // Two ranges [a, b) and [c, d) intersect iff a < d && c < b
        self.start < span.end && span.start < self.old_end
    }

    /// Check if this change completely contains a span.
    #[inline]
    pub fn contains(&self, span: Span) -> bool {
        self.start <= span.start && span.end <= self.old_end
    }

    /// Check if a span is entirely before this change.
    #[inline]
    pub fn is_before(&self, span: Span) -> bool {
        span.end <= self.start
    }

    /// Check if a span is entirely after this change.
    #[inline]
    pub fn is_after(&self, span: Span) -> bool {
        span.start >= self.old_end
    }

    /// New end position after the change is applied.
    #[inline]
    pub const fn new_end(&self) -> u32 {
        self.start + self.new_len
    }
}

/// Tracks which spans are affected by an edit.
///
/// The marker defines an "affected region" in the old text. Any AST node whose span
/// intersects this region must be reparsed. Nodes entirely before or after the region
/// can be reused with span adjustment.
///
/// # Lookahead Buffer
///
/// To handle parser lookahead, the affected region is typically extended backward
/// from the change start to include any tokens that might have been looked at
/// during the original parse. This follows TypeScript's incremental parsing approach.
///
/// # Examples
///
/// ```
/// use ori_ir::incremental::{ChangeMarker, TextChange};
/// use ori_ir::Span;
///
/// let change = TextChange::replace(100, 10, 15);
/// let marker = ChangeMarker::from_change(&change, 95); // prev token ended at 95
///
/// // Positions before the change region are unchanged
/// assert_eq!(marker.adjust_position(50), 50);
///
/// // Positions after the change region are shifted by delta
/// assert_eq!(marker.adjust_position(200), 205);
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChangeMarker {
    /// Start of the affected region (may be earlier than change.start due to lookahead).
    pub affected_start: u32,
    /// End of the affected region in the old text.
    pub affected_end: u32,
    /// Position adjustment delta for positions after the affected region.
    pub delta: i64,
}

impl ChangeMarker {
    /// Create a marker from a text change with lookahead extension.
    ///
    /// The `prev_token_end` parameter specifies where the previous token ended,
    /// which extends the affected region backward to handle parser lookahead.
    #[inline]
    pub fn from_change(change: &TextChange, prev_token_end: u32) -> Self {
        ChangeMarker {
            // Extend backward to handle lookahead - any token that might have
            // been peeked at during the original parse
            affected_start: prev_token_end.min(change.start),
            affected_end: change.old_end,
            delta: change.delta(),
        }
    }

    /// Create a marker directly from affected region and delta.
    #[inline]
    pub const fn new(affected_start: u32, affected_end: u32, delta: i64) -> Self {
        ChangeMarker {
            affected_start,
            affected_end,
            delta,
        }
    }

    /// Check if a span intersects the affected region.
    ///
    /// Spans that intersect the affected region must be reparsed.
    #[inline]
    pub fn intersects(&self, span: Span) -> bool {
        self.affected_start < span.end && span.start < self.affected_end
    }

    /// Check if a span is entirely before the affected region.
    ///
    /// Such spans can be reused without modification.
    #[inline]
    pub fn is_before(&self, span: Span) -> bool {
        span.end <= self.affected_start
    }

    /// Check if a span is entirely after the affected region.
    ///
    /// Such spans can be reused with position adjustment.
    #[inline]
    pub fn is_after(&self, span: Span) -> bool {
        span.start >= self.affected_end
    }

    /// Adjust a position from old text to new text.
    ///
    /// - Positions strictly before the affected region: unchanged
    /// - Positions at or after the affected end: shifted by delta
    /// - Positions inside the affected region (start <= pos < end): undefined in new text
    ///
    /// Note: For pure insertions where `affected_start == affected_end`, positions
    /// at that point are considered "at the affected region" and get shifted.
    #[inline]
    pub fn adjust_position(&self, pos: u32) -> u32 {
        if pos < self.affected_start {
            // Strictly before - unchanged
            pos
        } else if pos >= self.affected_end {
            // At or after the end of affected region - shift by delta
            // Safe: we check bounds and delta is computed from u32 values
            #[allow(
                clippy::cast_sign_loss,
                clippy::cast_possible_truncation,
                reason = "Bounds-checked: delta computed from u32 values"
            )]
            {
                (i64::from(pos) + self.delta) as u32
            }
        } else {
            // Inside affected region (start <= pos < end)
            // Position is invalid in new text; return unchanged for caller to handle
            pos
        }
    }

    /// Adjust a span from old text to new text.
    ///
    /// Returns `None` if the span intersects the affected region (must reparse).
    /// Returns `Some(adjusted_span)` if the span can be reused.
    #[inline]
    pub fn adjust_span(&self, span: Span) -> Option<Span> {
        if self.intersects(span) {
            None
        } else {
            Some(Span::new(
                self.adjust_position(span.start),
                self.adjust_position(span.end),
            ))
        }
    }
}

#[cfg(test)]
mod tests;
