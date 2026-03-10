//! Parsed type ID and range types.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Index into parsed type storage in arena.
///
/// Used to replace `Box<ParsedType>` with arena allocation for better
/// cache locality and reduced per-allocation overhead.
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct ParsedTypeId(u32);

impl ParsedTypeId {
    /// Invalid parsed type ID (sentinel value).
    pub const INVALID: ParsedTypeId = ParsedTypeId(u32::MAX);

    /// Create a new `ParsedTypeId`.
    #[inline]
    pub const fn new(index: u32) -> Self {
        ParsedTypeId(index)
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

impl Hash for ParsedTypeId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for ParsedTypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "ParsedTypeId({})", self.0)
        } else {
            write!(f, "ParsedTypeId::INVALID")
        }
    }
}

impl Default for ParsedTypeId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// Range of parsed types in flattened list.
///
/// Used for type argument lists like `Result<T, E>` → range of 2 types.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ParsedTypeRange {
    pub start: u32,
    pub len: u16,
}

impl ParsedTypeRange {
    /// Empty range.
    pub const EMPTY: ParsedTypeRange = ParsedTypeRange { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ParsedTypeRange { start, len }
    }

    /// Check if the range is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the number of types.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for ParsedTypeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ParsedTypeRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}
