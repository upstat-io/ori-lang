//! Lazy token capture for AST nodes.

use super::{Span, TokenList};

/// Lazy token capture for AST nodes that may need token access.
///
/// Instead of storing tokens directly (which would be expensive), this stores
/// indices into the cached `TokenList`. Access is O(1) via `TokenList::get_range()`.
///
/// # Use Cases
/// - **Formatters**: Know exact token boundaries for lossless roundtrip
/// - **Future macros**: Store token ranges for macro expansion
/// - **Attribute processing**: Preserve attribute syntax for IDE features
///
/// # Memory Efficiency
/// - `None` variant: 0 bytes discriminant (most common)
/// - `Range` variant: 8 bytes (start + end as u32)
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug, Default
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TokenCapture {
    /// No tokens captured (default for most nodes).
    #[default]
    None,

    /// Range of token indices `[start, end)` in the `TokenList`.
    ///
    /// Invariant: `start <= end`. An empty range has `start == end`.
    Range {
        /// Starting token index (inclusive).
        start: u32,
        /// Ending token index (exclusive).
        end: u32,
    },
}

impl TokenCapture {
    /// Create a new capture range.
    ///
    /// Returns `None` if the range is empty (start == end).
    #[inline]
    pub fn new(start: u32, end: u32) -> Self {
        debug_assert!(start <= end, "TokenCapture: start ({start}) > end ({end})");
        if start == end {
            Self::None
        } else {
            Self::Range { start, end }
        }
    }

    /// Check if this capture is empty (no tokens).
    #[inline]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Get the number of captured tokens.
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Range { start, end } => (end - start) as usize,
        }
    }

    /// Get the byte span covered by this capture.
    ///
    /// Returns `None` if the capture is empty or the token list is unavailable.
    #[inline]
    pub fn span(&self, tokens: &TokenList) -> Option<Span> {
        match self {
            Self::None => None,
            Self::Range { start, end } => {
                let first = tokens.get(*start as usize)?;
                let last = tokens.get((*end as usize).saturating_sub(1))?;
                Some(first.span.merge(last.span))
            }
        }
    }
}
