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
        /// Which variant is encoded by the niche value.
        ///
        /// For `Option<bool>`: None is variant 0 and uses niche value 2.
        /// The variant order in `EnumRepr.variants` matches the type checker's
        /// logical variant indices — this field eliminates the need to reorder
        /// variants for niche encoding.
        niche_variant_idx: u32,
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

/// Compute the minimum integer width needed to represent `variant_count` discriminants.
///
/// This is the canonical source of truth for tag width computation.
/// All consumers (codegen, canonical repr, ABI) derive the tag width from this function.
///
/// - 0 or 1 variants → `I8` (single variant may use `EnumTag::None` instead)
/// - 2–256 variants → `I8`
/// - 257–65536 variants → `I16`
/// - 65537–4294967296 variants → `I32`
/// - Larger → `I64`
#[must_use]
pub fn min_tag_width(variant_count: usize) -> IntWidth {
    match variant_count {
        0 | 1 => IntWidth::I8,
        n => {
            // Bits needed = ceil(log2(n)), computed with integer arithmetic:
            // (n - 1).leading_zeros() counts unused high bits in usize;
            // usize::BITS - leading_zeros = bits needed to represent 0..n-1.
            let bits_needed = usize::BITS - (n - 1).leading_zeros();
            match bits_needed {
                0..=8 => IntWidth::I8,
                9..=16 => IntWidth::I16,
                17..=32 => IntWidth::I32,
                _ => IntWidth::I64,
            }
        }
    }
}

impl VariantRepr {
    /// Whether this variant is a pointer type (for tagged pointer optimization).
    #[must_use]
    pub fn is_pointer(&self) -> bool {
        self.fields.len() == 1
            && matches!(
                &self.fields[0],
                MachineRepr::RcPointer(_)
                    | MachineRepr::FatPointer(_)
                    | MachineRepr::OpaquePtr
                    | MachineRepr::UnmanagedPtr
            )
    }
}
