//! Unified type index handle.
//!
//! `Idx` is THE canonical type representation.
//! All types are stored in a unified pool and referenced by their 32-bit index.
//!
//! # Design (from Zig's `InternPool`)
//!
//! - 32-bit indices allow 4+ billion unique types
//! - Primitive types have fixed indices (0-11) for O(1) lookup
//! - Type equality is O(1) index comparison
//! - Copy, lightweight passing

use std::fmt;

/// A 32-bit index into the type pool.
///
/// This is THE canonical type representation - no other type representation exists.
/// Types are compared by index equality (O(1)), not structural comparison.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct Idx(u32);

impl Idx {
    // Primitive Types (indices 0-11)
    // These are pre-interned at pool creation for O(1) access.

    /// The `int` type (64-bit signed integer).
    pub const INT: Self = Self(0);
    /// The `float` type (64-bit floating point).
    pub const FLOAT: Self = Self(1);
    /// The `bool` type.
    pub const BOOL: Self = Self(2);
    /// The `str` type (UTF-8 string).
    pub const STR: Self = Self(3);
    /// The `char` type (Unicode scalar value).
    pub const CHAR: Self = Self(4);
    /// The `byte` type (8-bit unsigned integer).
    pub const BYTE: Self = Self(5);
    /// The unit type `()`.
    pub const UNIT: Self = Self(6);
    /// The `Never` type (bottom type, no values).
    pub const NEVER: Self = Self(7);
    /// The error type (placeholder for type errors, propagates silently).
    pub const ERROR: Self = Self(8);
    /// The `Duration` type (time duration).
    pub const DURATION: Self = Self(9);
    /// The `Size` type (memory size/count).
    pub const SIZE: Self = Self(10);
    /// The `Ordering` type (comparison result: Less, Equal, Greater).
    pub const ORDERING: Self = Self(11);

    // Reserved Range (12-63)
    // Reserved for future primitive types.

    /// First index for dynamically allocated types.
    pub const FIRST_DYNAMIC: u32 = 64;

    /// Sentinel value indicating no type / invalid index.
    pub const NONE: Self = Self(u32::MAX);

    /// Number of pre-interned primitive types.
    pub const PRIMITIVE_COUNT: u32 = 12;

    /// Create an index from a raw u32 value.
    ///
    /// # Safety
    /// The caller must ensure the index is valid in the pool.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Get the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Check if this is a primitive type (pre-interned).
    #[inline]
    pub const fn is_primitive(self) -> bool {
        self.0 < Self::FIRST_DYNAMIC
    }

    /// Check if this is the NONE sentinel.
    #[inline]
    pub const fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    /// Check if this is the ERROR type.
    #[inline]
    pub const fn is_error(self) -> bool {
        self.0 == Self::ERROR.0
    }

    /// Check if this is the NEVER type.
    #[inline]
    pub const fn is_never(self) -> bool {
        self.0 == Self::NEVER.0
    }

    /// Check if this is the UNIT type.
    #[inline]
    pub const fn is_unit(self) -> bool {
        self.0 == Self::UNIT.0
    }

    /// Get the human-readable name for primitive types.
    ///
    /// Returns `Some("int")`, `Some("bool")`, etc. for known primitives,
    /// or `None` for dynamic (non-primitive) types that require a Pool
    /// to render their names.
    ///
    /// Casing matches actual Ori surface syntax: lowercase for the five
    /// keyword primitives (`int`/`float`/`bool`/`str`/`char`/`byte`/`()`),
    /// capitalized for the non-keyword type names (`Never`/`Duration`/
    /// `Size`/`Ordering`) — mirrors `ori_ir::TypeId::name()`, which shares
    /// this index layout for the same primitive slots.
    #[inline]
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0 => Some("int"),
            1 => Some("float"),
            2 => Some("bool"),
            3 => Some("str"),
            4 => Some("char"),
            5 => Some("byte"),
            6 => Some("()"),
            7 => Some("Never"),
            8 => Some("<error>"),
            9 => Some("Duration"),
            10 => Some("Size"),
            11 => Some("Ordering"),
            _ => None,
        }
    }

    /// Get the display name, using `"<type>"` as a fallback for dynamic types.
    ///
    /// Useful for error messages where a Pool is not available.
    #[inline]
    pub fn display_name(self) -> &'static str {
        self.name().unwrap_or("<type>")
    }
}

impl fmt::Debug for Idx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::INT => write!(f, "Idx::INT"),
            Self::FLOAT => write!(f, "Idx::FLOAT"),
            Self::BOOL => write!(f, "Idx::BOOL"),
            Self::STR => write!(f, "Idx::STR"),
            Self::CHAR => write!(f, "Idx::CHAR"),
            Self::BYTE => write!(f, "Idx::BYTE"),
            Self::UNIT => write!(f, "Idx::UNIT"),
            Self::NEVER => write!(f, "Idx::NEVER"),
            Self::ERROR => write!(f, "Idx::ERROR"),
            Self::DURATION => write!(f, "Idx::DURATION"),
            Self::SIZE => write!(f, "Idx::SIZE"),
            Self::ORDERING => write!(f, "Idx::ORDERING"),
            Self::NONE => write!(f, "Idx::NONE"),
            _ => write!(f, "Idx({})", self.0),
        }
    }
}

impl fmt::Display for Idx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Delegates to `name()` (the single canonical primitive-name table)
        // instead of a parallel literal match; only the two non-primitive
        // cases `name()` cannot classify are handled here.
        match self.name() {
            Some(name) => write!(f, "{name}"),
            None if *self == Self::NONE => write!(f, "<none>"),
            None => write!(f, "type#{}", self.0),
        }
    }
}

// Compile-time size assertion: Idx must be exactly 4 bytes
const _: () = assert!(std::mem::size_of::<Idx>() == 4);

#[cfg(test)]
mod tests;
