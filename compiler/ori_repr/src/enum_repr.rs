//! Enum representation types.
//!
//! These types describe the physical layout of enum (sum) types,
//! including discriminant strategy and per-variant payload layout.
//! They are populated by `canonical()` with explicit i64 tags,
//! then refined by §07 (niche filling, discriminant narrowing).

use ori_ir::Name;

use crate::repr::{IntWidth, MachineRepr};

/// Physical representation of an enum (sum) type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumRepr {
    /// Discriminant representation.
    pub tag: EnumTag,
    /// Per-variant payload representations.
    pub variants: Vec<VariantRepr>,
    /// Total size including tag and padding.
    pub size: u32,
    /// Alignment requirement.
    pub align: u32,
}

/// Discriminant encoding strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumTag {
    /// Explicit tag field at offset 0.
    Explicit {
        /// Width of the tag integer.
        width: IntWidth,
    },
    /// Niche — tag stored in invalid bit pattern of a field.
    Niche {
        /// Index of the field containing the niche.
        field_index: u32,
        /// Bit pattern used as the niche value.
        niche_value: u64,
    },
    /// No tag needed (single inhabited variant, e.g. newtype).
    None,
}

/// Physical representation of a single enum variant.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariantRepr {
    /// Variant name (interned).
    pub name: Name,
    /// Field representations (empty for unit variants).
    pub fields: Vec<MachineRepr>,
    /// Size of this variant's payload (excluding tag).
    pub size: u32,
    /// Alignment of this variant's payload.
    pub alignment: u32,
}

impl VariantRepr {
    /// Whether this variant is a pointer type (for tagged pointer optimization).
    #[must_use]
    pub fn is_pointer(&self) -> bool {
        self.fields.len() == 1
            && matches!(
                &self.fields[0],
                MachineRepr::RcPointer(_) | MachineRepr::FatPointer(_) | MachineRepr::OpaquePtr
            )
    }
}
