//! Match pattern and binding pattern ID and range types.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Index into match pattern storage in arena.
///
/// Used to replace `Box<MatchPattern>` with arena allocation for better
/// cache locality and reduced per-allocation overhead.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct MatchPatternId(u32);

impl MatchPatternId {
    /// Invalid match pattern ID (sentinel value).
    pub const INVALID: MatchPatternId = MatchPatternId(u32::MAX);

    /// Create a new `MatchPatternId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        MatchPatternId(index)
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
}

impl Hash for MatchPatternId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for MatchPatternId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "MatchPatternId({})", self.0)
        } else {
            write!(f, "MatchPatternId::INVALID")
        }
    }
}

impl Default for MatchPatternId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Range of match patterns in flattened list.
///
/// Used for pattern lists like `(a, b, c)` → range of 3 patterns.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct MatchPatternRange {
    pub start: u32,
    pub len: u16,
}

impl MatchPatternRange {
    /// Empty range.
    pub const EMPTY: MatchPatternRange = MatchPatternRange { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        MatchPatternRange { start, len }
    }

    /// Check if the range is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the number of patterns.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for MatchPatternRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MatchPatternRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Index into binding pattern storage in arena.
///
/// Used to replace inline `BindingPattern` in `ExprKind::Let` and `StmtKind::Let`
/// with arena allocation.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct BindingPatternId(u32);

impl BindingPatternId {
    /// Invalid binding pattern ID (sentinel value).
    pub const INVALID: BindingPatternId = BindingPatternId(u32::MAX);

    /// Create a new `BindingPatternId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        BindingPatternId(index)
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
}

impl Hash for BindingPatternId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for BindingPatternId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "BindingPatternId({})", self.0)
        } else {
            write!(f, "BindingPatternId::INVALID")
        }
    }
}

impl Default for BindingPatternId {
    fn default() -> Self {
        Self::INVALID
    }
}
