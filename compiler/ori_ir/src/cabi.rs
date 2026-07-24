//! C-ABI kind carrier for FFI `c_*` types.
//!
//! `CAbiKind` is the symbolic identity of an FFI C type (`c_int`, `c_long`,
//! ...). It is produced in `ori_types` at FFI type resolution and consumed in
//! `ori_repr` at the extern ABI boundary, which maps each kind to a concrete
//! `IntWidth` per the active target data-model. `ori_types` stays
//! target-agnostic (phase purity): it records WHICH c_* kind a type is; the
//! backend decides HOW WIDE on this target.

#[cfg(test)]
mod tests;

/// Symbolic C-ABI kind of an FFI `c_*` type.
///
/// Target-invariant kinds (`CChar`/`CShort`/`CInt`/`CLongLong`/`CFloat`/
/// `CDouble`) have a fixed width meaning; target-variant kinds (`CLong`/
/// `CSize`/`CPtr`) carry only the symbol — `ori_repr` resolves the concrete
/// width against the target data-model (LP64 vs LLP64).
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub enum CAbiKind {
    /// `c_char` — i8.
    CChar,
    /// `c_short` — i16.
    CShort,
    /// `c_int` — i32.
    CInt,
    /// `c_long` — i64 (LP64) / i32 (LLP64); target-resolved.
    CLong,
    /// `c_longlong` — i64.
    CLongLong,
    /// `c_size` — pointer width; target-resolved.
    CSize,
    /// `c_float` — f32.
    CFloat,
    /// `c_double` — f64.
    CDouble,
    /// `CPtr` — pointer width; target-resolved.
    CPtr,
}

impl CAbiKind {
    /// Every C-ABI kind, in declaration order. The canonical inventory of FFI
    /// `c_*` kinds: `from_name` and the type checker's Name-keyed resolver both
    /// derive from this rather than re-listing the variants.
    pub const ALL: [Self; 9] = [
        Self::CChar,
        Self::CShort,
        Self::CInt,
        Self::CLong,
        Self::CLongLong,
        Self::CSize,
        Self::CFloat,
        Self::CDouble,
        Self::CPtr,
    ];

    /// The surface type name of this kind (`c_int`, `CPtr`, ...).
    ///
    /// The single SSOT for the kind <-> name pairing: `from_name` inverts it
    /// and the type checker's `WellKnownNames` builds its interned `Name` table
    /// from it (no scattered c_* lists). Exhaustive match — a new variant
    /// fails to compile until named here.
    #[must_use]
    pub const fn c_name(self) -> &'static str {
        match self {
            Self::CChar => "c_char",
            Self::CShort => "c_short",
            Self::CInt => "c_int",
            Self::CLong => "c_long",
            Self::CLongLong => "c_longlong",
            Self::CSize => "c_size",
            Self::CFloat => "c_float",
            Self::CDouble => "c_double",
            Self::CPtr => "CPtr",
        }
    }

    /// Map an FFI C type name to its `CAbiKind`. `None` for any non-FFI name.
    ///
    /// Inverts [`Self::c_name`] over [`Self::ALL`] — the name pairing lives in
    /// `c_name` alone.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.c_name() == name)
    }

    /// `true` for kinds whose concrete width depends on the target data-model
    /// (`CLong`/`CSize`/`CPtr`); `ori_repr` MUST resolve these against the
    /// active target rather than assuming a fixed width.
    #[must_use]
    pub fn is_target_variant(self) -> bool {
        matches!(self, Self::CLong | Self::CSize | Self::CPtr)
    }

    /// `true` for the C floating-point kinds (`CFloat`/`CDouble`). The single
    /// source of the float/integer split: `ori_types` derives the ergonomic
    /// `Idx::FLOAT`/`Idx::INT` resolution from this rather than re-enumerating
    /// the c_* names.
    #[must_use]
    pub fn is_float(self) -> bool {
        matches!(self, Self::CFloat | Self::CDouble)
    }
}
