//! Tagged pointer analysis for enum representation optimization.
//!
//! On 64-bit systems, heap pointers have alignment ≥8, meaning the low 3 bits
//! are always zero. These bits can store a 3-bit tag (up to 8 variants).
//!
//! Only single-word pointer types are taggable:
//! - `RcPointer` (heap-allocated, 8-byte aligned)
//! - `OpaquePtr` (C interop pointer)
//! - `UnmanagedPtr` (raw pointer)
//!
//! Multi-word types (`FatPointer`, `Closure`, `Struct`, `Tuple`) are NOT
//! taggable — they occupy more than 8 bytes.
//!
//! Scalar payloads (`int`, `bool`, `float`, `byte`, etc.) are NOT taggable —
//! their low bits carry data that the `value & ~0x7` decode would destroy.
//!
//! This module provides [`is_taggable_pointer`] to classify a payload type
//! and [`can_use_tagged_pointer`] to check if an enum layout is eligible for
//! tagged pointer optimization.

use crate::enum_repr::{EnumRepr, EnumTag};
use crate::repr::MachineRepr;

/// Maximum variant count addressable by a 3-bit tag.
///
/// The low 3 bits of an aligned pointer are zero on 64-bit systems with
/// 8-byte alignment, leaving exactly 8 distinct tag values.
const MAX_TAG_VARIANTS: usize = 8;

/// Size in bytes of a tagged-pointer encoded enum value.
///
/// The entire enum is a single 64-bit slot containing the encoded
/// `(pointer | tag)` value. There is no separate tag field and no
/// payload struct.
const TAGGED_PTR_SIZE: u32 = 8;

/// Alignment in bytes of a tagged-pointer encoded enum value.
///
/// 8-byte alignment is required so the low 3 bits of the slot are
/// available for the discriminant. Any aligned pointer (which `ori_rt`
/// guarantees) satisfies this.
const TAGGED_PTR_ALIGN: u32 = 8;

/// Check if a variant payload is a single-word pointer suitable for tagging.
///
/// `FatPointer` (str, [T], {K:V}, Set<T>) is 24 bytes — NOT taggable.
/// Scalar payloads are NOT taggable: their low bits carry data that the
/// `value & ~0x7` decode would corrupt (e.g., masking `int(5)` gives `0`).
///
/// Only single-word pointer types qualify:
/// - [`MachineRepr::RcPointer`] — heap-allocated, alignment ≥ 8
/// - [`MachineRepr::OpaquePtr`] — C interop pointer
/// - [`MachineRepr::UnmanagedPtr`] — raw pointer
///
/// **Recursive types are excluded.** [`canonical_inner`] emits the recursive
/// cycle marker as `RcPointer { inner: OpaquePtr }`, which has different
/// boxing/load semantics from a regular heap pointer (the value at the heap
/// address is itself an encoded tagged-pointer i64, not a value of the
/// pointer's `inner` type). Supporting recursive tagged pointers requires
/// box-and-load codegen for Construct/Project — out of scope for §07.3.A.
/// Tracked as a future extension; the analysis layer rejects them so the
/// codegen path never sees a recursive tagged-pointer enum.
///
/// [`canonical_inner`]: crate::canonical
#[must_use]
pub(crate) fn is_taggable_pointer(repr: &MachineRepr) -> bool {
    match repr {
        MachineRepr::RcPointer(rc) => {
            // Reject the cycle marker (recursive enum field). The marker is
            // produced by `canonical_inner` when it detects a cycle and
            // takes the form `RcPointer { inner: OpaquePtr, .. }`. A
            // non-recursive `RcPointer` wraps a meaningful inner type
            // (struct, tuple, etc.), but in practice user enums never
            // produce that shape — recursive fields are the only path
            // through which `RcPointer` reaches a variant payload here.
            !matches!(*rc.inner, MachineRepr::OpaquePtr)
        }
        MachineRepr::OpaquePtr | MachineRepr::UnmanagedPtr => true,
        _ => false,
    }
}

/// Check if an enum is eligible for tagged pointer optimization.
///
/// An enum qualifies when:
/// 1. It has at most 8 variants (3-bit tag).
/// 2. Every non-unit variant has exactly one single-word pointer field.
/// 3. At least one variant has a pointer payload (otherwise no benefit).
///
/// Unit variants (no payload) are permitted — they carry only the tag value.
/// Multi-field variants are excluded: the tagged pointer encoding stores the
/// tag in the low bits of the *single* payload word, so a variant with two or
/// more fields cannot be encoded.
///
/// # Spec
///
/// This is the §07.3 eligibility check. The tagged pointer encoding is:
/// ```text
/// [63:3] pointer value  [2:0] tag
/// ```
/// Encode: `ptr | tag` (low 3 bits of `ptr` are 0 due to alignment).
/// Decode tag: `value & 0x7`. Decode pointer: `value & !0x7`.
#[must_use]
pub(crate) fn can_use_tagged_pointer(enum_repr: &EnumRepr) -> bool {
    // Constraint 1: at most 8 variants for 3-bit tag.
    if enum_repr.variants.len() > MAX_TAG_VARIANTS {
        return false;
    }

    // Constraint 2: every non-unit variant has exactly one single-word pointer.
    let all_variants_taggable = enum_repr
        .variants
        .iter()
        .all(|v| v.fields.is_empty() || (v.fields.len() == 1 && is_taggable_pointer(&v.fields[0])));
    if !all_variants_taggable {
        return false;
    }

    // Constraint 3: at least one pointer variant — otherwise no benefit.
    enum_repr
        .variants
        .iter()
        .any(|v| v.fields.len() == 1 && is_taggable_pointer(&v.fields[0]))
}

/// Apply tagged-pointer optimization to an enum representation.
///
/// Takes a candidate `EnumRepr` (typically the explicit-tag default
/// produced by `canonical_enum()`) and returns the tagged-pointer-encoded
/// form when [`can_use_tagged_pointer`] returns true. Otherwise the input
/// is returned unchanged.
///
/// The returned `EnumRepr` has:
/// - `tag = EnumTag::TaggedPtr`
/// - `size = 8` (single i64 slot, no padding)
/// - `align = 8` (required for 3-bit tag space)
/// - `variants` unchanged (per-variant pointer/unit role is read from
///   `VariantRepr.fields` at codegen time)
///
/// This function is the canonical constructor of `EnumTag::TaggedPtr`.
/// All `EnumTag::TaggedPtr` values must originate here, mirroring how
/// `optimize_option_repr`/`optimize_result_repr` are the canonical
/// constructors of `EnumTag::Niche`.
///
/// # Spec
///
/// §07.3 — tagged pointer encoding for enums where every variant is
/// either unit or carries a single single-word pointer payload.
#[must_use]
pub(crate) fn optimize_tagged_ptr_repr(candidate: EnumRepr) -> EnumRepr {
    if !can_use_tagged_pointer(&candidate) {
        return candidate;
    }

    EnumRepr {
        tag: EnumTag::TaggedPtr,
        variants: candidate.variants,
        size: TAGGED_PTR_SIZE,
        align: TAGGED_PTR_ALIGN,
    }
}
