//! Enum representation types.
//!
//! These types describe the physical layout of enum (sum) types,
//! including discriminant strategy and per-variant payload layout.
//! They are populated by `canonical()` with explicit i64 tags,
//! then refined by niche filling and discriminant narrowing.

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
///
/// # Construction
///
/// `EnumTag` should only be constructed in:
/// - `canonical::type_repr::canonical_enum()` (initial explicit/tagless tag)
/// - `layout::niche::optimize_option_repr()` / `optimize_result_repr()` (niche tags)
///
/// Consumers should use predicate methods (`is_niche()`, `is_tagless()`,
/// `needs_tag_field()`, `payload_gep_index()`) rather than matching variants.
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

impl EnumTag {
    /// Whether this is a niche-encoded tag.
    #[must_use]
    pub fn is_niche(&self) -> bool {
        matches!(self, Self::Niche { .. })
    }

    /// Whether this is a tagless (single-variant) enum.
    #[must_use]
    pub fn is_tagless(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Whether the enum has a dedicated tag field (explicit encoding).
    #[must_use]
    pub fn needs_tag_field(&self) -> bool {
        matches!(self, Self::Explicit { .. })
    }

    /// GEP index for the payload in the LLVM struct.
    ///
    /// - `Explicit`: payload is at index 1 (after tag at index 0)
    /// - `Niche` / `None`: payload starts at index 0 (no tag field)
    #[must_use]
    pub fn payload_gep_index(&self) -> u32 {
        match self {
            Self::Explicit { .. } => 1,
            Self::Niche { .. } | Self::None => 0,
        }
    }
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
/// - 0 or 1 variants → `I8` (single variant uses `EnumTag::None` instead,
///   bypassing this function entirely — see `canonical_enum()`)
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
    /// Whether this variant's payload is a single pointer type.
    ///
    /// Used by tagged pointer optimization to identify variants where the
    /// tag can be stored in pointer alignment bits. Not relevant for
    /// discriminant narrowing or niche filling.
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
