//! Canonical enum layout facts.
//!
//! [`EnumLayoutInfo`] is the single source of truth for every enum layout
//! question a codegen consumer can ask — tag encoding, GEP indices for tag
//! and payload, per-field payload offsets, ABI size, and slot count.
//! Consumers query this struct instead of computing layout independently.

#[cfg(test)]
mod tests;

use crate::enum_repr::{EnumRepr, EnumTag, VariantRepr};
use crate::layout::{field_size, slot_count, slot_padded_size};

/// Canonical source of truth for enum layout facts.
///
/// Computed (post-optimization) from the final [`EnumRepr`] via
/// [`compute_enum_layout_info`].
#[derive(Clone, Debug, PartialEq)]
pub struct EnumLayoutInfo {
    /// Tag encoding strategy (Explicit, Niche, `TaggedPtr`, None).
    pub tag_encoding: EnumTag,
    /// Number of variants (the tag value range).
    pub variant_count: usize,
    /// GEP index for the tag field (None for Niche/TaggedPtr/None encodings).
    pub tag_gep_index: Option<u32>,
    /// GEP index for the payload field (None for all-unit enums and `TaggedPtr`).
    pub payload_gep_index: Option<u32>,
    /// Byte offsets of each payload field within the payload region, at
    /// i64-slot boundaries. Indexed by variant field index (0-based). Computed
    /// for the first maximum-payload variant — the variant that sizes the
    /// payload region.
    pub payload_field_offsets: Vec<u32>,
    /// Total ABI size in bytes (tag + payload + padding).
    pub abi_size: u64,
    /// Payload i64-slot count (0 for all-unit enums).
    pub payload_slot_count: u64,
    /// Whether this is an Option/Result carrying a runtime ABI contract.
    pub is_runtime_abi: bool,
}

impl EnumLayoutInfo {
    /// Whether the enum has a dedicated tag field (`EnumTag::Explicit`).
    #[must_use]
    pub fn has_explicit_tag(&self) -> bool {
        matches!(self.tag_encoding, EnumTag::Explicit { .. })
    }

    /// Whether the tag is niche-encoded (`EnumTag::Niche`).
    #[must_use]
    pub fn has_niche(&self) -> bool {
        self.tag_encoding.is_niche()
    }

    /// Whether the enum is tagged-pointer encoded (`EnumTag::TaggedPtr`) —
    /// no struct layout; consumers must not GEP.
    #[must_use]
    pub fn is_tagged_ptr(&self) -> bool {
        self.tag_encoding.is_tagged_ptr()
    }

    /// Whether the enum is tagless (`EnumTag::None`, single-variant newtype).
    #[must_use]
    pub fn is_tagless(&self) -> bool {
        self.tag_encoding.is_tagless()
    }

    /// Whether `field_index` refers to the tag field.
    ///
    /// True only for an explicit-tag enum's field 0; false for every field of
    /// a niche / tagged-pointer / tagless enum (no separate tag field).
    #[must_use]
    pub fn field_is_tag(&self, field_index: u32) -> bool {
        self.tag_gep_index == Some(field_index)
    }

    /// Byte offset of payload field `variant_field_idx` within the payload
    /// region (i64-slot boundary), or `None` when the index is out of range.
    #[must_use]
    pub fn payload_slot_offset(&self, variant_field_idx: usize) -> Option<u64> {
        self.payload_field_offsets
            .get(variant_field_idx)
            .copied()
            .map(u64::from)
    }
}

/// Compute [`EnumLayoutInfo`] from the final [`EnumRepr`].
///
/// Single computation point for all enum layout facts. Handles all four
/// `EnumTag` variants:
///
/// - `Explicit { width }` — tag at GEP 0, payload at GEP 1.
/// - `Niche { .. }` — no explicit tag, payload occupies the full struct (GEP 0).
/// - `TaggedPtr` — discriminant in the pointer's low bits, no struct GEPs.
/// - `None` — single variant, no tag, payload is the type itself (GEP 0).
///
/// `is_runtime_abi` is supplied by the caller (it requires the type pool to
/// detect Option/Result).
#[must_use]
pub fn compute_enum_layout_info(repr: &EnumRepr, is_runtime_abi: bool) -> EnumLayoutInfo {
    let tag_gep_index = match repr.tag {
        EnumTag::Explicit { .. } => Some(0),
        EnumTag::Niche { .. } | EnumTag::TaggedPtr | EnumTag::None => None,
    };

    // A tagged pointer has no struct payload region — the value is a single
    // tagged slot, so it carries no GEP, no field offsets, and zero slots.
    let is_tagged_ptr = matches!(repr.tag, EnumTag::TaggedPtr);
    let has_payload = repr.variants.iter().any(|v| !v.fields.is_empty());
    let payload_gep_index = if !has_payload || is_tagged_ptr {
        None
    } else {
        // 1 for Explicit (after the tag), 0 for Niche/None (payload IS the struct).
        Some(repr.tag.payload_gep_index())
    };

    // The first maximum-payload variant sizes the payload region; its fields
    // give the canonical i64-slot offsets. Tagged pointers have no such region.
    let representative = if is_tagged_ptr {
        None
    } else {
        repr.variants
            .iter()
            .fold(None::<&VariantRepr>, |best, v| match best {
                Some(b) if b.size >= v.size => Some(b),
                _ => Some(v),
            })
    };

    let mut payload_field_offsets = Vec::new();
    if let Some(v) = representative {
        let mut offset: u64 = 0;
        for f in &v.fields {
            payload_field_offsets.push(u32::try_from(offset).unwrap_or(u32::MAX));
            offset = offset.saturating_add(slot_padded_size(u64::from(field_size(f))));
        }
    }

    let payload_slot_count = representative.map_or(0, |v| slot_count(u64::from(v.size)));

    EnumLayoutInfo {
        tag_encoding: repr.tag,
        variant_count: repr.variants.len(),
        tag_gep_index,
        payload_gep_index,
        payload_field_offsets,
        abi_size: u64::from(repr.size),
        payload_slot_count,
        is_runtime_abi,
    }
}
