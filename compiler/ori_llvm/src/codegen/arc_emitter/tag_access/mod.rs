//! Tag encoding/decoding abstraction for enum representation optimization (§07).
//!
//! Active since §07.2 — consumed by niche-aware `SetTag`, `Project`, `Switch`,
//! drop, and RC codegen paths.
//!
//! [`TagEncoding`] encapsulates the logic of how an enum's discriminant is
//! physically encoded in memory. All codegen consumers use this instead of
//! hardcoding `const_i64(tag)` / `struct_gep(ptr, 0)` / `load(i64, ...)`.
//!
//! Four encoding strategies:
//! - **Explicit** — dedicated tag field at GEP index 0 (current default)
//! - **Niche** — tag encoded as an invalid bit pattern in a payload field
//! - **`TaggedPtr`** — tag stored in low 3 bits of an aligned pointer (§07.3)
//! - **None** — single-variant enum, no tag needed
//!
//! `TaggedPtr` enums are special: they have NO struct layout. The entire
//! enum is a single 64-bit value. Consumers must check
//! [`TagEncoding::is_tagged_ptr`] BEFORE attempting any GEP-based access.

#[cfg(test)]
mod tests;

use ori_repr::{EnumRepr, EnumTag, IntWidth};

/// Pure encoding information for an enum's tag — no LLVM dependency.
///
/// Constructed from an [`EnumRepr`] and answers all questions about how to
/// read, write, and switch on the discriminant. Codegen consumers query this
/// instead of making assumptions about tag layout.
///
/// Used by all `arc_emitter` consumers starting in §07.1 (discriminant narrowing).
#[derive(Debug, Clone)]
pub(crate) struct TagEncoding {
    tag: EnumTag,
    variant_count: u32,
}

#[allow(dead_code, reason = "§07.3/§07.4 will consume remaining methods")]
impl TagEncoding {
    /// Create a `TagEncoding` from an `EnumRepr`.
    pub(crate) fn from_enum_repr(repr: &EnumRepr) -> Self {
        Self {
            tag: repr.tag,
            variant_count: repr.variants.len() as u32,
        }
    }

    /// Create a `TagEncoding` directly from an `EnumTag` and variant count.
    pub(crate) fn new(tag: EnumTag, variant_count: u32) -> Self {
        Self { tag, variant_count }
    }

    /// The integer width of the explicit tag, if any.
    ///
    /// - `Explicit { width }` → `Some(width)`
    /// - `Niche { .. }` → `None` (no separate tag field)
    /// - `TaggedPtr` → `None` (tag is in pointer alignment bits, not a field)
    /// - `None` → `None` (no tag at all)
    pub(crate) fn tag_width(&self) -> Option<IntWidth> {
        match &self.tag {
            EnumTag::Explicit { width } => Some(*width),
            EnumTag::Niche { .. } | EnumTag::TaggedPtr | EnumTag::None => None,
        }
    }

    /// The GEP field index where the tag is stored, if an explicit tag exists.
    ///
    /// - `Explicit` → `Some(0)` (tag is always field 0 in `{ tag, payload }`)
    /// - `Niche` → `None` (tag is encoded in a payload field, not a separate field)
    /// - `TaggedPtr` → `None` (no struct layout — entire value is `i64`)
    /// - `None` → `None`
    pub(crate) fn tag_gep_index(&self) -> Option<u32> {
        match &self.tag {
            EnumTag::Explicit { .. } => Some(0),
            EnumTag::Niche { .. } | EnumTag::TaggedPtr | EnumTag::None => None,
        }
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

    /// The GEP field index where the payload starts for a given variant.
    ///
    /// - `Explicit` → 1 (payload follows the tag field)
    /// - `Niche` → 0 (no tag field — payload IS the entire struct)
    /// - `TaggedPtr` → 0, but **GEP is invalid** — tagged-pointer enums have
    ///   no struct layout. Consumers MUST check [`is_tagged_ptr`] first.
    /// - `None` → 0 (no tag field — single variant is the entire struct)
    pub(crate) fn payload_gep_index(&self) -> u32 {
        match &self.tag {
            EnumTag::Explicit { .. } => 1,
            EnumTag::Niche { .. } | EnumTag::TaggedPtr | EnumTag::None => 0,
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

    /// Whether this is a niche-encoded enum.
    pub(crate) fn is_niche(&self) -> bool {
        matches!(&self.tag, EnumTag::Niche { .. })
    }

    /// Whether this is a tagged-pointer enum (§07.3).
    ///
    /// Tagged-pointer enums have NO struct layout — the entire enum is a
    /// single 64-bit value. Consumers must check this BEFORE any GEP-based
    /// access (`tag_gep_index`, `payload_gep_index` are documented to return
    /// 0, but the value is meaningless for `TaggedPtr`).
    pub(crate) fn is_tagged_ptr(&self) -> bool {
        matches!(&self.tag, EnumTag::TaggedPtr)
    }

    /// Whether this is a single-variant (no-tag) enum.
    pub(crate) fn is_tagless(&self) -> bool {
        matches!(&self.tag, EnumTag::None)
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

    /// The number of variants.
    pub(crate) fn variant_count(&self) -> u32 {
        self.variant_count
    }

    /// For niche encoding: which field contains the niche.
    pub(crate) fn niche_field_index(&self) -> Option<u32> {
        match &self.tag {
            EnumTag::Niche { field_index, .. } => Some(*field_index),
            _ => None,
        }
    }

    /// For niche encoding: the niche sentinel value.
    pub(crate) fn niche_value(&self) -> Option<u64> {
        match &self.tag {
            EnumTag::Niche { niche_value, .. } => Some(*niche_value),
            _ => None,
        }
    }

    /// For niche encoding: which variant is encoded by the niche value.
    pub(crate) fn niche_variant_idx(&self) -> Option<u32> {
        match &self.tag {
            EnumTag::Niche {
                niche_variant_idx, ..
            } => Some(*niche_variant_idx),
            _ => None,
        }
    }
}
