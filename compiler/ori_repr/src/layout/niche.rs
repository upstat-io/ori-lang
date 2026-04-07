//! Niche analysis for enum representation optimization.
//!
//! A "niche" is an invalid bit pattern in a type. If an enum variant's payload
//! has a niche, we can use it to encode a different variant, eliminating the
//! explicit tag.
//!
//! Examples:
//! - `bool` has 254 niches (values 2..=255)
//! - `Ordering` has 253 niches (values 3..=255)
//! - `char` has ~4 billion niches (0x110000..=0xFFFFFFFF)
//! - `RcPointer` has 1 niche (null, since heap pointers are always non-null)
//! - `str` has 1 niche (null data pointer, since SSO ensures non-zero)
//!
//! This module provides:
//! - [`find_niches`] to discover available niches for a given [`MachineRepr`]
//! - [`find_enum_niches`] to discover niches in nested enums
//! - [`optimize_option_repr`] to apply niche filling to `Option<T>`
//! - [`optimize_result_repr`] to apply niche filling to `Result<T, E>`

use ori_ir::Name;

use crate::enum_repr::{EnumRepr, EnumTag, VariantRepr};
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::FatRepr;

use super::{field_align, field_size, repr_align, repr_size};

/// A niche (invalid bit pattern) discovered in a type's representation.
///
/// Used to eliminate explicit discriminant tags in enum layouts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Niche {
    /// Which field contains the niche (for fat pointers: 2 = data ptr).
    pub field_index: u32,
    /// Byte offset within the field (currently always 0).
    pub offset: u32,
    /// Number of available niche values.
    pub available: u128,
    /// Starting value of the niche range.
    pub start: u128,
}

/// Discover available niches in a type's representation.
///
/// Returns niches sorted by usefulness (fewest available first — prefer
/// single-value niches like null pointers over large ranges like char).
pub(crate) fn find_niches(repr: &MachineRepr) -> Vec<Niche> {
    match repr {
        // bool: values 0 and 1 → niche at value 2..=255 (254 niches)
        MachineRepr::Bool => vec![Niche {
            field_index: 0,
            offset: 0,
            available: 254,
            start: 2,
        }],

        // Ordering: values 0,1,2 → niche at 3..=255 (253 niches)
        MachineRepr::Ordering => vec![Niche {
            field_index: 0,
            offset: 0,
            available: 253,
            start: 3,
        }],

        // Reference/pointer: null (0) is never a valid heap pointer
        // (ori_rc_alloc guarantees non-null, min 8-byte aligned).
        MachineRepr::RcPointer(_) => vec![Niche {
            field_index: 0,
            offset: 0,
            available: 1,
            start: 0,
        }],

        // Fat pointer — ONLY str has a null-ptr niche.
        // str uses SSO for empty strings (OriStr::EMPTY has SSO_FLAG set in
        // byte 23, making the data-pointer-slot always non-zero). Therefore
        // null data pointer (all-zero in bytes 16-23) is an invalid str.
        //
        // [T], {K:V}, Set<T> use {0, 0, null} for empty collections, so
        // null data pointer IS a valid value — NO niche available.
        MachineRepr::FatPointer(FatRepr::Str) => vec![Niche {
            field_index: 2,
            offset: 0,
            available: 1,
            start: 0,
        }],

        // Char: Unicode codepoints are 0..=0x10FFFF (21 bits).
        // Values 0x110000..=0xFFFFFFFF are invalid (huge niche space).
        MachineRepr::Char => vec![Niche {
            field_index: 0,
            offset: 0,
            available: u128::from(0xFFFF_FFFFu32 - 0x10_FFFFu32),
            start: 0x11_0000,
        }],

        // Nested enum: unused discriminant values are niches.
        MachineRepr::Enum(e) => find_enum_niches(e),

        // No niche available:
        // - Byte: all 256 values valid
        // - Int: all values valid (without range info from range analysis)
        // - Float: NaN-based niches too complex (IEEE 754)
        // - Collections: null data ptr is valid (empty list/map/set)
        // - Duration/Size/Unit/Never/Struct/Tuple/Closure/Range/Ptr: no niche
        MachineRepr::Byte
        | MachineRepr::Int { .. }
        | MachineRepr::Float { .. }
        | MachineRepr::FatPointer(FatRepr::Collection { .. } | FatRepr::Map { .. })
        | MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::Struct(_)
        | MachineRepr::Tuple(_)
        | MachineRepr::Closure(_)
        | MachineRepr::Range
        | MachineRepr::StackPromoted { .. }
        | MachineRepr::OpaquePtr
        | MachineRepr::UnmanagedPtr => vec![],
    }
}

/// Discover niches in a nested enum from unused discriminant values.
///
/// If an enum has N variants and a tag with width W, the unused tag values
/// `N..=(2^(W*8) - 1)` are niches. For niche-encoded enums, niches come
/// from the niche field's remaining capacity.
pub(crate) fn find_enum_niches(e: &EnumRepr) -> Vec<Niche> {
    match &e.tag {
        EnumTag::Explicit { width } => {
            let max_val = match width {
                IntWidth::I8 => 255u128,
                IntWidth::I16 => 65_535,
                IntWidth::I32 => 4_294_967_295,
                IntWidth::I64 => u128::from(u64::MAX),
            };
            let variant_count = u128::from(e.variants.len() as u64);
            if variant_count <= max_val {
                let available = max_val - variant_count + 1;
                vec![Niche {
                    field_index: 0, // tag is at field 0 for explicit tags
                    offset: 0,
                    available,
                    start: variant_count,
                }]
            } else {
                vec![]
            }
        }
        EnumTag::Niche {
            field_index,
            niche_value,
            ..
        } => {
            // A niche-encoded enum has used one niche value. Check if the
            // underlying field has more niches available.
            let data_variant = e.variants.iter().find(|v| !v.fields.is_empty());
            if let Some(variant) = data_variant {
                let fi = *field_index as usize;
                if fi < variant.fields.len() {
                    let field_niches = find_niches(&variant.fields[fi]);
                    // Filter out the niche value already used by this enum.
                    field_niches
                        .into_iter()
                        .filter_map(|n| {
                            let niche_val = u128::from(*niche_value);
                            if n.start <= niche_val && niche_val < n.start + n.available {
                                // This niche range contains the used value.
                                // Split: values after the used one.
                                let used_offset = niche_val - n.start + 1;
                                let remaining = n.available - used_offset;
                                if remaining > 0 {
                                    Some(Niche {
                                        field_index: n.field_index,
                                        offset: n.offset,
                                        available: remaining,
                                        start: n.start + used_offset,
                                    })
                                } else {
                                    None
                                }
                            } else {
                                // Niche range doesn't overlap — fully available.
                                Some(n)
                            }
                        })
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        EnumTag::None => {
            // Single-variant enum — check if the variant's payload has niches.
            if let Some(variant) = e.variants.first() {
                if variant.fields.len() == 1 {
                    find_niches(&variant.fields[0])
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        }
        EnumTag::TaggedPtr => {
            // §07.3: Tagged-pointer enums encode the discriminant in the
            // low 3 bits of an 8-byte-aligned pointer slot. All 8 tag values
            // are reserved for variant discriminators (the eligibility check
            // caps the variant count at 8). The high 61 bits hold either a
            // heap pointer or zero for unit variants.
            //
            // No niches are exposed: an outer enum cannot reuse the inner
            // tagged-pointer value space without a custom analysis that
            // understands which (tag, high-bits) pairs are unreachable.
            // That analysis is intentionally out of scope for §07.3.
            vec![]
        }
    }
}

/// Build `Option<T>` variant representations: `(Some(T), None, some_size, some_align)`.
///
/// Shared between [`optimize_option_repr`] and [`default_option_repr_public`].
/// Variant order: Some=0, None=1 (matches type checker).
fn build_option_variants(inner: &MachineRepr) -> (VariantRepr, VariantRepr, u32, u32) {
    let some_size = field_size(inner);
    let some_align = field_align(inner);
    let some_variant = VariantRepr {
        name: Name::new(0, 0), // "Some" — variant 0
        fields: vec![inner.clone()],
        size: some_size,
        alignment: some_align,
    };
    let none_variant = VariantRepr {
        name: Name::new(0, 1), // "None" — variant 1
        fields: vec![],
        size: 0,
        alignment: 1,
    };
    (some_variant, none_variant, some_size, some_align)
}

/// Build `Result<T, E>` variant representations.
///
/// Returns `(Ok(T), Err(E), ok_size, ok_align, err_size, err_align)`.
/// Shared between [`optimize_result_repr`] and [`default_result_repr_public`].
/// Variant order: Ok=0, Err=1 (matches type checker).
fn build_result_variants(
    ok_repr: &MachineRepr,
    err_repr: &MachineRepr,
) -> (VariantRepr, VariantRepr, u32, u32, u32, u32) {
    let ok_size = field_size(ok_repr);
    let ok_align = field_align(ok_repr);
    let ok_variant = VariantRepr {
        name: Name::new(0, 0), // "Ok"
        fields: vec![ok_repr.clone()],
        size: ok_size,
        alignment: ok_align,
    };
    let err_size = field_size(err_repr);
    let err_align = field_align(err_repr);
    let err_variant = VariantRepr {
        name: Name::new(0, 1), // "Err"
        fields: vec![err_repr.clone()],
        size: err_size,
        alignment: err_align,
    };
    (
        ok_variant,
        err_variant,
        ok_size,
        ok_align,
        err_size,
        err_align,
    )
}

/// Optimize `Option<T>` layout using niche filling.
///
/// If the inner type has a niche, `None` is encoded as the niche value
/// (eliminating the explicit tag). Otherwise falls back to the standard
/// `{ i64 tag, T payload }` layout.
///
/// Variant order matches the type checker: Some=0, None=1
/// (Ori prelude: `type Option<T> = Some(T) | None`).
pub(crate) fn optimize_option_repr(inner: &MachineRepr) -> MachineRepr {
    let (some_variant, none_variant, some_size, some_align) = build_option_variants(inner);

    let niches = find_niches(inner);
    if let Some(niche) = niches.first() {
        if niche.available >= 1 {
            // Niche optimization: None is encoded as the niche value.
            // No explicit tag field — the payload type IS the enum type.
            let size = repr_size(inner);
            let align = repr_align(inner);
            return MachineRepr::Enum(EnumRepr {
                tag: EnumTag::Niche {
                    field_index: niche.field_index,
                    niche_value: u64::try_from(niche.start).unwrap_or(u64::MAX),
                    niche_variant_idx: 1, // None is variant 1 (type checker order)
                },
                variants: vec![some_variant, none_variant],
                size,
                align,
            });
        }
    }

    // Fallback: explicit I64 tag (runtime-compatible layout).
    default_option_repr(some_variant, none_variant, some_size, some_align)
}

/// Optimize `Result<T, E>` layout using niche filling.
///
/// If one payload has a niche, the other variant is encoded via that niche.
/// If both have niches, prefer the one that eliminates more padding.
///
/// Variant order matches the type checker: Ok=0, Err=1.
pub(crate) fn optimize_result_repr(ok_repr: &MachineRepr, err_repr: &MachineRepr) -> MachineRepr {
    let (ok_variant, err_variant, ok_size, ok_align, err_size, err_align) =
        build_result_variants(ok_repr, err_repr);

    let ok_niches = find_niches(ok_repr);
    let err_niches = find_niches(err_repr);

    // Try Ok's niche first (Err variant encoded via Ok's niche).
    if let Some(niche) = ok_niches.first() {
        if niche.available >= 1 {
            let size = repr_size(ok_repr).max(repr_size(err_repr));
            let align = repr_align(ok_repr).max(repr_align(err_repr));
            return MachineRepr::Enum(EnumRepr {
                tag: EnumTag::Niche {
                    field_index: niche.field_index,
                    niche_value: u64::try_from(niche.start).unwrap_or(u64::MAX),
                    niche_variant_idx: 1, // Err uses Ok's niche
                },
                variants: vec![ok_variant, err_variant],
                size,
                align,
            });
        }
    }

    // Try Err's niche (Ok variant encoded via Err's niche).
    if let Some(niche) = err_niches.first() {
        if niche.available >= 1 {
            let size = repr_size(ok_repr).max(repr_size(err_repr));
            let align = repr_align(ok_repr).max(repr_align(err_repr));
            return MachineRepr::Enum(EnumRepr {
                tag: EnumTag::Niche {
                    field_index: niche.field_index,
                    niche_value: u64::try_from(niche.start).unwrap_or(u64::MAX),
                    niche_variant_idx: 0, // Ok uses Err's niche
                },
                variants: vec![ok_variant, err_variant],
                size,
                align,
            });
        }
    }

    // Fallback: explicit I64 tag.
    default_result_repr(
        ok_variant,
        err_variant,
        ok_size,
        ok_align,
        err_size,
        err_align,
    )
}

/// Explicit-tag Option layout — public entry point for `canonical_option()`
/// when `NICHE_CODEGEN_READY` is false.
///
/// Variant order: Some=0, None=1 (matches type checker).
pub(crate) fn default_option_repr_public(inner: &MachineRepr) -> MachineRepr {
    let (some_variant, none_variant, some_size, some_align) = build_option_variants(inner);
    default_option_repr(some_variant, none_variant, some_size, some_align)
}

/// Explicit-tag Result layout — public entry point for `canonical_result()`
/// when `NICHE_CODEGEN_READY` is false.
pub(crate) fn default_result_repr_public(
    ok_repr: &MachineRepr,
    err_repr: &MachineRepr,
) -> MachineRepr {
    let (ok_variant, err_variant, ok_size, ok_align, err_size, err_align) =
        build_result_variants(ok_repr, err_repr);
    default_result_repr(
        ok_variant,
        err_variant,
        ok_size,
        ok_align,
        err_size,
        err_align,
    )
}

/// Standard explicit-tag Option layout (runtime-compatible).
///
/// Variant order: Some=0, None=1 (matches type checker).
fn default_option_repr(
    some_variant: VariantRepr,
    none_variant: VariantRepr,
    some_size: u32,
    some_align: u32,
) -> MachineRepr {
    let tag_width = IntWidth::I64;
    let (size, align) = super::compute_explicit_tag_layout(tag_width, some_size, some_align);
    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit { width: tag_width },
        variants: vec![some_variant, none_variant],
        size,
        align,
    })
}

/// Standard explicit-tag Result layout (runtime-compatible).
fn default_result_repr(
    ok_variant: VariantRepr,
    err_variant: VariantRepr,
    ok_size: u32,
    ok_align: u32,
    err_size: u32,
    err_align: u32,
) -> MachineRepr {
    let tag_width = IntWidth::I64;
    let max_payload = ok_size.max(err_size);
    let max_variant_align = ok_align.max(err_align);
    let (size, align) =
        super::compute_explicit_tag_layout(tag_width, max_payload, max_variant_align);
    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit { width: tag_width },
        variants: vec![ok_variant, err_variant],
        size,
        align,
    })
}
