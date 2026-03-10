//! Function sequence and function expression ID types.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Index into function sequence storage in arena.
///
/// Used to replace inline `FunctionSeq` in `ExprKind` with arena allocation,
/// reducing `ExprKind` size by ~56 bytes (`FunctionSeq`'s largest variant).
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct FunctionSeqId(u32);

impl FunctionSeqId {
    /// Invalid function sequence ID (sentinel value).
    pub const INVALID: FunctionSeqId = FunctionSeqId(u32::MAX);

    /// Create a new `FunctionSeqId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        FunctionSeqId(index)
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

impl Hash for FunctionSeqId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for FunctionSeqId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "FunctionSeqId({})", self.0)
        } else {
            write!(f, "FunctionSeqId::INVALID")
        }
    }
}

impl Default for FunctionSeqId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Index into function expression (map/filter/fold/etc.) storage in arena.
///
/// Used to replace inline `FunctionExp` in `ExprKind` with arena allocation.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct FunctionExpId(u32);

impl FunctionExpId {
    /// Invalid function expression ID (sentinel value).
    pub const INVALID: FunctionExpId = FunctionExpId(u32::MAX);

    /// Create a new `FunctionExpId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        FunctionExpId(index)
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

impl Hash for FunctionExpId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for FunctionExpId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "FunctionExpId({})", self.0)
        } else {
            write!(f, "FunctionExpId::INVALID")
        }
    }
}

impl Default for FunctionExpId {
    fn default() -> Self {
        Self::INVALID
    }
}
