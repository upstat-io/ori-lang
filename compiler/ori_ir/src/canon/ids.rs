//! ID and range newtypes for the canonical IR.
//!
//! These types provide type-safe indices into [`CanArena`](super::CanArena)
//! storage, preventing accidental cross-use with `ExprId`/`ExprRange` from
//! the parse-level AST.

use std::fmt;
use std::hash::{Hash, Hasher};

/// Index into a [`CanArena`](super::CanArena). Distinct from
/// [`ExprId`](crate::ExprId) — these reference canonical expressions
/// in a separate index space.
///
/// # Salsa Compatibility
/// Implements `Copy`, `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct CanId(u32);

impl CanId {
    /// Sentinel value indicating "no expression" (analogous to `ExprId::INVALID`).
    /// Used for optional child expressions (e.g., no else branch, no guard).
    pub const INVALID: CanId = CanId(u32::MAX);

    /// Create a new `CanId` from a raw index.
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Get the raw index into the arena.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// Get the raw `u32` value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Bridge: create a `CanId` from an `ExprId`'s raw index.
    ///
    /// Used by backends that haven't migrated to `CanonResult` yet (`ori_arc`).
    /// The resulting `CanId` carries the same raw index as the `ExprId`, which
    /// the backend interprets in its own context.
    ///
    /// This will be removed once the ARC backend migrates to `CanonResult` (07.2).
    #[inline]
    pub const fn from_expr_id(id: crate::ExprId) -> Self {
        Self(id.raw())
    }

    /// Bridge: convert back to an `ExprId` raw index.
    ///
    /// Used by backends that haven't migrated to `CanonResult` yet.
    /// Will be removed once all backends use `CanonResult` (07.2).
    #[inline]
    pub const fn to_expr_id(self) -> crate::ExprId {
        crate::ExprId::new(self.0)
    }

    /// Returns `true` if this is a valid (non-sentinel) ID.
    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

impl Hash for CanId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for CanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::INVALID {
            write!(f, "CanId::INVALID")
        } else {
            write!(f, "CanId({})", self.0)
        }
    }
}

impl Default for CanId {
    fn default() -> Self {
        Self::INVALID
    }
}

/// A contiguous range of canonical expression IDs in a [`CanArena`](super::CanArena).
///
/// Used for expression lists: function arguments, list elements, block
/// statements, tuple elements, etc. Indexes into the arena's `expr_lists`
/// storage.
///
/// Layout matches [`ExprRange`](crate::ExprRange): `start: u32, len: u16` = 8 bytes.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanRange {
    pub start: u32,
    pub len: u16,
}

impl CanRange {
    /// Empty range constant.
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    /// Create a new range.
    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    /// Returns `true` if the range contains no elements.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of elements in the range.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Range of map entries in a [`CanArena`](super::CanArena). Each entry is a key-value pair.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanMapEntryRange {
    pub start: u32,
    pub len: u16,
}

impl CanMapEntryRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanMapEntryRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanMapEntryRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Range of struct field initializers in a [`CanArena`](super::CanArena). Each field is name + value.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanFieldRange {
    pub start: u32,
    pub len: u16,
}

impl CanFieldRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanFieldRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanFieldRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Index into a [`CanArena`](super::CanArena)'s binding pattern storage.
///
/// Replaces `BindingPatternId` (which indexes `ExprArena.binding_patterns`)
/// with a canonical equivalent that keeps the IR self-contained.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct CanBindingPatternId(u32);

impl CanBindingPatternId {
    #[inline]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for CanBindingPatternId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CanBindingPatternId({})", self.0)
    }
}

/// Range of binding pattern IDs in `CanArena.binding_pattern_lists`.
///
/// Used for `Tuple` and `List` sub-patterns which contain multiple children.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanBindingPatternRange {
    pub start: u32,
    pub len: u16,
}

impl CanBindingPatternRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanBindingPatternRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanBindingPatternRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}

/// Range of field bindings in `CanArena.field_bindings`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CanFieldBindingRange {
    pub start: u32,
    pub len: u16,
}

impl CanFieldBindingRange {
    pub const EMPTY: Self = Self { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        Self { start, len }
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}

impl fmt::Debug for CanFieldBindingRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CanFieldBindingRange({}..{})",
            self.start,
            self.start + u32::from(self.len)
        )
    }
}
