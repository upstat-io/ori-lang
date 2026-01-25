//! Arena Range Types
//!
//! All range types for arena-allocated data. These are compact representations
//! that store start index and length, enabling efficient iteration over arena data.
//!
//! # Salsa Compatibility
//! All types have Copy, Clone, Eq, PartialEq, Hash, Debug for Salsa requirements.

use std::fmt;

/// Range of parameters in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ParamRange {
    pub start: u32,
    pub len: u16,
}

impl ParamRange {
    pub const EMPTY: ParamRange = ParamRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ParamRange { start, len }
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

impl fmt::Debug for ParamRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ParamRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of generic parameters in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct GenericParamRange {
    pub start: u32,
    pub len: u16,
}

impl GenericParamRange {
    pub const EMPTY: GenericParamRange = GenericParamRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        GenericParamRange { start, len }
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

impl fmt::Debug for GenericParamRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GenericParamRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of match arms in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct ArmRange {
    pub start: u32,
    pub len: u16,
}

impl ArmRange {
    pub const EMPTY: ArmRange = ArmRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ArmRange { start, len }
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

impl fmt::Debug for ArmRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ArmRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of map entries in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct MapEntryRange {
    pub start: u32,
    pub len: u16,
}

impl MapEntryRange {
    pub const EMPTY: MapEntryRange = MapEntryRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        MapEntryRange { start, len }
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

impl fmt::Debug for MapEntryRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MapEntryRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of field initializers in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct FieldInitRange {
    pub start: u32,
    pub len: u16,
}

impl FieldInitRange {
    pub const EMPTY: FieldInitRange = FieldInitRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        FieldInitRange { start, len }
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

impl fmt::Debug for FieldInitRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FieldInitRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of sequence bindings in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct SeqBindingRange {
    pub start: u32,
    pub len: u16,
}

impl SeqBindingRange {
    pub const EMPTY: SeqBindingRange = SeqBindingRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        SeqBindingRange { start, len }
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

impl fmt::Debug for SeqBindingRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SeqBindingRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of named expressions in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct NamedExprRange {
    pub start: u32,
    pub len: u16,
}

impl NamedExprRange {
    pub const EMPTY: NamedExprRange = NamedExprRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        NamedExprRange { start, len }
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

impl fmt::Debug for NamedExprRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NamedExprRange({}..{})", self.start, self.start + self.len as u32)
    }
}

/// Range of call arguments in the arena.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct CallArgRange {
    pub start: u32,
    pub len: u16,
}

impl CallArgRange {
    pub const EMPTY: CallArgRange = CallArgRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        CallArgRange { start, len }
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

impl fmt::Debug for CallArgRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CallArgRange({}..{})", self.start, self.start + self.len as u32)
    }
}
