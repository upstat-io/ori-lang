//! Struct, tuple, field, RC, fat pointer, and closure representation types.
//!
//! These types describe the physical layout of compound types in generated
//! code. They are populated by `canonical()` with declaration-order layouts,
//! then refined by §06 (struct layout optimization) and §09 (ARC header
//! compression).

use ori_ir::Name;

use crate::repr::{FloatWidth, IntWidth, MachineRepr};

/// Physical representation of a struct type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructRepr {
    /// Fields in optimized memory order (may differ from declaration order).
    pub fields: Vec<FieldRepr>,
    /// Total size in bytes (including padding).
    pub size: u32,
    /// Alignment requirement.
    pub align: u32,
    /// Whether all fields are trivial (no RC needed).
    pub trivial: bool,
}

/// Physical representation of a single struct or tuple field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldRepr {
    /// Original field name (interned via `Name` from `ori_ir`).
    ///
    /// Required by §06 for: (1) debug symbol emission (DWARF needs names),
    /// (2) C-ABI verification (declaration order must match for `#repr("c")`),
    /// (3) tracing output (audit trail logs field names, not indices).
    pub name: Name,
    /// Original field index in declaration order (0-based).
    ///
    /// `fields[i].original_index` tells codegen the source order after
    /// §06 may have reordered `fields` by alignment/size.
    pub original_index: u32,
    /// Offset in bytes from struct start (set by §06 layout algorithm).
    pub offset: u32,
    /// Machine representation of this field.
    pub repr: MachineRepr,
}

/// Physical representation of a tuple type (anonymous struct).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TupleRepr {
    /// Element representations in optimized memory order.
    pub elements: Vec<FieldRepr>,
    /// Total size in bytes.
    pub size: u32,
    /// Alignment requirement.
    pub align: u32,
    /// Whether all elements are trivial (no RC needed).
    pub trivial: bool,
}

/// Physical representation of a reference-counted heap allocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RcRepr {
    /// Width of the reference count header.
    pub rc_width: IntWidth,
    /// Whether RC operations are atomic.
    pub atomic: bool,
    /// The inner data representation.
    pub inner: Box<MachineRepr>,
    /// Whether this is stack-promotable (escape analysis).
    pub stack_promotable: bool,
}

/// Fat pointer representation distinguishing collection vs string vs map.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FatRepr {
    /// String: `{i64 len, i64 cap, ptr data}`.
    Str,
    /// Single-element collection (`[T]`, `Set<T>`): `{i64 len, i64 cap, ptr data}`.
    Collection {
        /// Machine representation of collection elements.
        element_repr: Box<MachineRepr>,
    },
    /// Map (`{K: V}`): `{i64 len, i64 cap, ptr data}` — retains both key and
    /// value representations for drop functions and element layout.
    Map {
        /// Machine representation of map keys.
        key_repr: Box<MachineRepr>,
        /// Machine representation of map values.
        value_repr: Box<MachineRepr>,
    },
}

/// Physical representation of a function/closure type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClosureRepr {
    /// Parameter representations.
    pub params: Vec<MachineRepr>,
    /// Return representation.
    pub ret: Box<MachineRepr>,
}

impl IntWidth {
    /// Size in bytes of this integer width.
    #[must_use]
    pub const fn size_bytes(self) -> u32 {
        match self {
            Self::I8 => 1,
            Self::I16 => 2,
            Self::I32 => 4,
            Self::I64 => 8,
        }
    }

    /// Alignment in bytes of this integer width.
    #[must_use]
    pub const fn alignment(self) -> u32 {
        self.size_bytes()
    }
}

impl FloatWidth {
    /// Size in bytes of this float width.
    #[must_use]
    pub const fn size_bytes(self) -> u32 {
        match self {
            Self::F32 => 4,
            Self::F64 => 8,
        }
    }

    /// Alignment in bytes of this float width.
    #[must_use]
    pub const fn alignment(self) -> u32 {
        self.size_bytes()
    }
}
