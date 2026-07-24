//! Tag encoding/decoding abstraction for enum representation optimization.
//!
//! Active since consumed by niche-aware `SetTag`, `Project`, `Switch`,
//! drop, and RC codegen paths.
//!
//! [`TagEncoding`] encapsulates the logic of how an enum's discriminant is
//! physically encoded in memory. All codegen consumers use this instead of
//! hardcoding `const_i64(tag)` / `struct_gep(ptr, 0)` / `load(i64,...)`.
//!
//! Four encoding strategies:
//! - **Explicit** — dedicated tag field at GEP index 0 (current default)
//! - **Niche** — tag encoded as an invalid bit pattern in a payload field
//! - **`TaggedPtr`** — tag stored in low 3 bits of an aligned pointer
//! - **None** — single-variant enum, no tag needed
//!
//! `TaggedPtr` enums are special: they have no struct layout. The entire enum
//! is a single 64-bit value, so consumers must avoid GEP-based access.

#[cfg(test)]
mod tests;

use ori_repr::{EnumRepr, EnumTag};

/// Pure encoding information for an enum's tag — no LLVM dependency.
///
/// Constructed from an [`EnumRepr`] and answers all questions about how to
/// read, write, and switch on the discriminant. Codegen consumers query this
/// instead of making assumptions about tag layout.
///
/// Used by all `arc_emitter` consumers starting (discriminant narrowing).
#[derive(Debug, Clone)]
pub(crate) struct TagEncoding {
    tag: EnumTag,
}

impl TagEncoding {
    /// Create a `TagEncoding` from an `EnumRepr`.
    pub(crate) fn from_enum_repr(repr: &EnumRepr) -> Self {
        Self { tag: repr.tag }
    }

    #[cfg(test)]
    fn new(tag: EnumTag, _variant_count: u32) -> Self {
        Self { tag }
    }

    /// Convert a logical variant index to the physical tag value to store.
    ///
    /// - `Explicit` → identity (variant 0 = tag 0, variant 1 = tag 1, etc.)
    /// - `Niche` → for the niche variant, returns the niche value; for others,
    ///   the value is implicit in the payload (no separate store needed).
    /// - `TaggedPtr` → identity (`variant_idx` is the low-3-bits tag value).
    ///   The full encoded value is `(payload_ptr | variant_idx)`; this method
    ///   returns just the tag bits — the OR with the pointer happens at the
    ///   construction site via [`TagEncoding::tagged_ptr_encode_const`].
    /// - `None` → always 0 (but `store_tag` is a no-op for `None`).
    pub(crate) fn variant_to_tag_value(&self, variant_idx: u32) -> u64 {
        match &self.tag {
            // `Explicit` and `TaggedPtr` both encode the discriminant as the
            // variant index — `Explicit` stores it as a separate tag field,
            // `TaggedPtr` ORs it into the low 3 bits of the encoded value.
            // Either way, the numeric tag value is `variant_idx` itself.
            EnumTag::Explicit { .. } | EnumTag::TaggedPtr => u64::from(variant_idx),
            EnumTag::Niche {
                niche_value,
                niche_variant_idx,
                ..
            } => {
                if variant_idx == *niche_variant_idx {
                    *niche_value
                } else {
                    // Non-niche variants: the payload IS the value.
                    // No separate tag store — the caller stores the payload directly.
                    u64::from(variant_idx)
                }
            }
            EnumTag::None => 0,
        }
    }

    /// Whether a `store_tag` call is needed for a given variant.
    ///
    /// - `Explicit` → always true (every variant needs its tag stored)
    /// - `Niche` → true ONLY for the niche variant (others encode via payload)
    /// - `TaggedPtr` → false (the tag is OR'd into the encoded value at
    ///   construction time, not stored as a separate field)
    /// - `None` → false (no tag to store)
    pub(crate) fn needs_tag_store(&self, variant_idx: u32) -> bool {
        match &self.tag {
            EnumTag::Explicit { .. } => true,
            EnumTag::Niche {
                niche_variant_idx, ..
            } => variant_idx == *niche_variant_idx,
            EnumTag::TaggedPtr | EnumTag::None => false,
        }
    }

    /// Bit mask for extracting the discriminant from a tagged-pointer encoded
    /// value: `value & TAGGED_PTR_TAG_MASK` yields the variant index.
    ///
    /// 8-byte alignment guarantees the low 3 bits of any pointer are zero,
    /// leaving exactly 8 distinct tag values.
    pub(crate) const TAGGED_PTR_TAG_MASK: u64 = 0x7;

    /// Bit mask for extracting the pointer from a tagged-pointer encoded
    /// value: `value & TAGGED_PTR_PTR_MASK` yields the original aligned
    /// pointer (or zero for unit variants).
    pub(crate) const TAGGED_PTR_PTR_MASK: u64 = !0x7;

    /// For niche encoding: which field contains the niche.
    pub(crate) fn niche_field_index(&self) -> Option<u32> {
        match &self.tag {
            EnumTag::Niche { field_index, .. } => Some(*field_index),
            _ => None,
        }
    }

    /// Return the three fields that are present together on a niche encoding.
    pub(crate) fn niche_fields(&self) -> Option<(u32, u64, u32)> {
        match &self.tag {
            EnumTag::Niche {
                field_index,
                niche_value,
                niche_variant_idx,
            } => Some((*field_index, *niche_value, *niche_variant_idx)),
            _ => None,
        }
    }
}
