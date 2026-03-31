// §07.0 scaffolding — consumers migrate to TagEncoding in §07.1.
// Remove this allow when the first consumer is migrated.
#![allow(dead_code, reason = "§07.0 scaffolding — consumers migrate in §07.1")]

//! Tag encoding/decoding abstraction for enum representation optimization (§07).
//!
//! [`TagEncoding`] encapsulates the logic of how an enum's discriminant is
//! physically encoded in memory. All codegen consumers use this instead of
//! hardcoding `const_i64(tag)` / `struct_gep(ptr, 0)` / `load(i64, ...)`.
//!
//! Three encoding strategies:
//! - **Explicit** — dedicated tag field at GEP index 0 (current default: i64)
//! - **Niche** — tag encoded as an invalid bit pattern in a payload field
//! - **None** — single-variant enum, no tag needed

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
    /// - `None` → `None` (no tag at all)
    pub(crate) fn tag_width(&self) -> Option<IntWidth> {
        match &self.tag {
            EnumTag::Explicit { width } => Some(*width),
            EnumTag::Niche { .. } | EnumTag::None => None,
        }
    }

    /// The GEP field index where the tag is stored, if an explicit tag exists.
    ///
    /// - `Explicit` → `Some(0)` (tag is always field 0 in `{ tag, payload }`)
    /// - `Niche` → `None` (tag is encoded in a payload field, not a separate field)
    /// - `None` → `None`
    pub(crate) fn tag_gep_index(&self) -> Option<u32> {
        match &self.tag {
            EnumTag::Explicit { .. } => Some(0),
            EnumTag::Niche { .. } | EnumTag::None => None,
        }
    }

    /// Convert a logical variant index to the physical tag value to store.
    ///
    /// - `Explicit` → identity (variant 0 = tag 0, variant 1 = tag 1, etc.)
    /// - `Niche` → for the niche variant, returns the niche value; for others,
    ///   the value is implicit in the payload (no separate store needed).
    /// - `None` → always 0 (but `store_tag` is a no-op for `None`).
    pub(crate) fn variant_to_tag_value(&self, variant_idx: u32) -> u64 {
        match &self.tag {
            EnumTag::Explicit { .. } => u64::from(variant_idx),
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
    /// - `None` → 0 (no tag field — single variant is the entire struct)
    pub(crate) fn payload_gep_index(&self) -> u32 {
        match &self.tag {
            EnumTag::Explicit { .. } => 1,
            EnumTag::Niche { .. } | EnumTag::None => 0,
        }
    }

    /// Whether a `store_tag` call is needed for a given variant.
    ///
    /// - `Explicit` → always true (every variant needs its tag stored)
    /// - `Niche` → true ONLY for the niche variant (others encode via payload)
    /// - `None` → false (no tag to store)
    pub(crate) fn needs_tag_store(&self, variant_idx: u32) -> bool {
        match &self.tag {
            EnumTag::Explicit { .. } => true,
            EnumTag::Niche {
                niche_variant_idx, ..
            } => variant_idx == *niche_variant_idx,
            EnumTag::None => false,
        }
    }

    /// Whether this is a niche-encoded enum.
    pub(crate) fn is_niche(&self) -> bool {
        matches!(&self.tag, EnumTag::Niche { .. })
    }

    /// Whether this is a single-variant (no-tag) enum.
    pub(crate) fn is_tagless(&self) -> bool {
        matches!(&self.tag, EnumTag::None)
    }

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
