//! Expression ID and range types.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Index into expression arena.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
///
/// # Design
/// Per design: "No `Box<Expr>`, use `ExprId(u32)` indices"
/// - Memory: 4 bytes (vs 8 bytes for Box)
/// - Equality: O(1) integer compare
/// - Cache locality: indices into contiguous array
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct ExprId(u32);

impl ExprId {
    /// Invalid expression ID (sentinel value).
    pub const INVALID: ExprId = ExprId(u32::MAX);

    /// Create a new `ExprId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        ExprId(index)
    }

    /// Get the index into the arena.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Get the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this is a valid ID.
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }

    /// Check if this ID represents a present (non-sentinel) value.
    ///
    /// Alias for `is_valid()` — used when `ExprId` replaces `Option<ExprId>`
    /// to make intent clearer at call sites.
    #[inline]
    pub const fn is_present(self) -> bool {
        self.0 != u32::MAX
    }
}

impl Hash for ExprId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for ExprId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "ExprId({})", self.0)
        } else {
            write!(f, "ExprId::INVALID")
        }
    }
}

impl Default for ExprId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Range of expressions in flattened list.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
///
/// # Design
/// Per design spec: uses (start: u32, len: u16) = 6 bytes logical.
/// Rust aligns to 8 bytes, still 3x better than `Vec<ExprId>` at 24+ bytes.
/// - start: u32 (4 bytes) - start index in `expr_lists`
/// - len: u16 (2 bytes) - number of expressions
/// - padding: 2 bytes (Rust alignment)
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct ExprRange {
    pub start: u32,
    pub len: u16,
}

impl ExprRange {
    /// Empty range.
    pub const EMPTY: ExprRange = ExprRange { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ExprRange { start, len }
    }

    /// Check if the range is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the number of expressions.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Iterator over indices in this range.
    #[inline]
    pub fn indices(&self) -> impl Iterator<Item = u32> {
        self.start..(self.start + u32::from(self.len))
    }
}

impl fmt::Debug for ExprRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExprRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

impl Default for ExprRange {
    fn default() -> Self {
        Self::EMPTY
    }
}
