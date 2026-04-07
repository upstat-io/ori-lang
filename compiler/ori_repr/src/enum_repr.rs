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
/// - `layout::tagged_ptr::optimize_tagged_ptr_repr()` (§07.3.A — tagged pointer tag)
///
/// Consumers should use predicate methods (`is_niche()`, `is_tagless()`,
/// `is_tagged_ptr()`, `needs_tag_field()`, `payload_gep_index()`) rather than
/// matching variants.
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
    /// Tagged pointer — discriminant encoded in the low 3 bits of an
    /// 8-byte-aligned pointer slot. Spec: §07.3 (tagged pointer optimization).
    ///
    /// The enum value is a single 64-bit slot:
    /// `[63:3] pointer (or 0 for unit variants)  [2:0] variant index`
    ///
    /// Encode: `ptr | tag` (low 3 bits of `ptr` are 0 due to 8-byte alignment).
    /// Decode tag: `value & 0x7`. Decode pointer: `value & !0x7`.
    ///
    /// Only valid for enums where every variant is either unit (no payload)
    /// or carries a single single-word pointer field (`RcPointer`,
    /// `OpaquePtr`, or `UnmanagedPtr`), with at most 8 variants. The
    /// per-variant pointer-vs-unit distinction is read from
    /// [`VariantRepr::is_pointer`] (`fields.is_empty()` ⇒ unit). No
    /// per-enum data is needed because the encoding is uniform.
    ///
    /// Eligibility is checked by [`crate::layout::tagged_ptr::can_use_tagged_pointer`].
    TaggedPtr,
    /// No tag needed (single inhabited variant, e.g. newtype).
    None,
}

impl EnumTag {
    /// Whether this is a niche-encoded tag.
    #[must_use]
    pub fn is_niche(&self) -> bool {
        matches!(self, Self::Niche { .. })
    }

    /// Whether this is a tagged-pointer encoded tag (§07.3).
    ///
    /// Tagged-pointer enums have NO struct layout — the entire enum is a
    /// single 64-bit value with the discriminant in the low 3 bits.
    /// Consumers must check this BEFORE attempting any GEP-based access.
    #[must_use]
    pub fn is_tagged_ptr(&self) -> bool {
        matches!(self, Self::TaggedPtr)
    }

    /// Whether this is a tagless (single-variant) enum.
    ///
    /// Note: this returns `false` for `TaggedPtr` even though tagged-pointer
    /// enums have no separate tag *field*. "Tagless" specifically means
    /// "single inhabited variant, struct IS the payload". Tagged pointers
    /// are multi-variant — use [`is_tagged_ptr`] instead.
    #[must_use]
    pub fn is_tagless(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Whether the enum has a dedicated tag field (explicit encoding).
    ///
    /// Returns `true` only for `Explicit`. `Niche`, `TaggedPtr`, and `None`
    /// all encode the discriminant inline (in a payload niche, in pointer
    /// alignment bits, or implicitly via the single-variant rule).
    #[must_use]
    pub fn needs_tag_field(&self) -> bool {
        matches!(self, Self::Explicit { .. })
    }

    /// GEP index for the payload in the LLVM struct.
    ///
    /// - `Explicit`: payload is at index 1 (after tag at index 0)
    /// - `Niche` / `None`: payload starts at index 0 (no tag field)
    /// - `TaggedPtr`: returns 0, but **GEP is not valid** — tagged pointer
    ///   enums have no struct layout. Consumers must check
    ///   [`is_tagged_ptr`] first and use mask-based decode instead.
    #[must_use]
    pub fn payload_gep_index(&self) -> u32 {
        match self {
            Self::Explicit { .. } => 1,
            Self::Niche { .. } | Self::TaggedPtr | Self::None => 0,
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
