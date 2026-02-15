//! Interned type identifier.
//!
//! `TypeId` is the parser-level type representation. It uses the same index layout
//! as `ori_types::Idx` so that primitive types 0-11 map by identity between the two.
//!
//! # Pre-interned Types
//! Primitive types have fixed indices matching `Idx`:
//! - INT=0, FLOAT=1, BOOL=2, STR=3, CHAR=4, BYTE=5
//! - UNIT=6, NEVER=7, ERROR=8, DURATION=9, SIZE=10, ORDERING=11
//!
//! # Special Markers
//! - INFER (12): Placeholder during inference — never stored in the type pool
//! - `SELF_TYPE` (13): Self type in trait/impl contexts — never stored in the type pool
//!
//! # Salsa Compatibility
//! Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug

use std::fmt;
use std::hash::{Hash, Hasher};

/// Interned type identifier.
///
/// 32-bit index with the same layout as `ori_types::Idx` for primitive types.
/// Compound types start at index 64 (matching `Idx::FIRST_DYNAMIC`).
///
/// # Salsa Compatibility
/// Has all required traits: Copy, Clone, Eq, `PartialEq`, Hash, Debug
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct TypeId(u32);

impl TypeId {
    // Primitive Types (indices 0-11, matching Idx)

    pub const INT: TypeId = TypeId(0);
    pub const FLOAT: TypeId = TypeId(1);
    pub const BOOL: TypeId = TypeId(2);
    pub const STR: TypeId = TypeId(3);
    pub const CHAR: TypeId = TypeId(4);
    pub const BYTE: TypeId = TypeId(5);
    /// Unit type `()`. Alias: `VOID`.
    pub const UNIT: TypeId = TypeId(6);
    pub const NEVER: TypeId = TypeId(7);
    /// Error type placeholder (propagates silently through type checking).
    pub const ERROR: TypeId = TypeId(8);
    /// Duration (nanoseconds).
    pub const DURATION: TypeId = TypeId(9);
    /// Size (bytes/count).
    pub const SIZE: TypeId = TypeId(10);
    /// Ordering type (Less | Equal | Greater), represented as i8 in LLVM.
    pub const ORDERING: TypeId = TypeId(11);

    // Special Markers (12-13, NOT stored in type pool)

    /// Placeholder during type inference. Never appears in final types.
    pub const INFER: TypeId = TypeId(12);
    /// Self type in trait/impl contexts. Never appears in final types.
    pub const SELF_TYPE: TypeId = TypeId(13);

    /// Legacy alias for `UNIT`. Prefer `UNIT` in new code.
    pub const VOID: TypeId = Self::UNIT;

    /// First ID for dynamically allocated compound types.
    /// Matches `Idx::FIRST_DYNAMIC` so raw values are interchangeable.
    pub const FIRST_COMPOUND: u32 = 64;

    /// Number of pre-interned primitive types (0-11).
    pub const PRIMITIVE_COUNT: u32 = 12;

    /// Get the raw u32 value.
    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Create from a raw u32 value.
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        TypeId(raw)
    }

    /// Check if this is a primitive type (indices 0-11).
    #[inline]
    pub const fn is_primitive(self) -> bool {
        self.0 < Self::PRIMITIVE_COUNT
    }

    /// Check if this is the inference placeholder.
    #[inline]
    pub const fn is_infer(self) -> bool {
        self.0 == Self::INFER.0
    }

    /// Check if this is the Self type (used in trait/impl contexts).
    #[inline]
    pub const fn is_self_type(self) -> bool {
        self.0 == Self::SELF_TYPE.0
    }

    /// Check if this is the error type.
    #[inline]
    pub const fn is_error(self) -> bool {
        self.0 == Self::ERROR.0
    }

    /// Get the display name for primitive types.
    ///
    /// Returns `Some(name)` for pre-interned primitive types,
    /// `None` for markers, error, and compound types.
    #[must_use]
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
}

impl Hash for TypeId {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl fmt::Debug for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::INT => write!(f, "TypeId::INT"),
            Self::FLOAT => write!(f, "TypeId::FLOAT"),
            Self::BOOL => write!(f, "TypeId::BOOL"),
            Self::STR => write!(f, "TypeId::STR"),
            Self::CHAR => write!(f, "TypeId::CHAR"),
            Self::BYTE => write!(f, "TypeId::BYTE"),
            Self::UNIT => write!(f, "TypeId::UNIT"),
            Self::NEVER => write!(f, "TypeId::NEVER"),
            Self::ERROR => write!(f, "TypeId::ERROR"),
            Self::DURATION => write!(f, "TypeId::DURATION"),
            Self::SIZE => write!(f, "TypeId::SIZE"),
            Self::ORDERING => write!(f, "TypeId::ORDERING"),
            Self::INFER => write!(f, "TypeId::INFER"),
            Self::SELF_TYPE => write!(f, "TypeId::SELF_TYPE"),
            _ => write!(f, "TypeId({})", self.0),
        }
    }
}

impl Default for TypeId {
    fn default() -> Self {
        Self::INFER
    }
}

#[cfg(test)]
mod tests;
