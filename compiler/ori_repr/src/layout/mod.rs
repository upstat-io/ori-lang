//! Layout computation utilities for `MachineRepr`.
//!
//! Provides ABI-correct size, alignment, and layout computation for
//! machine representations. Used by [`crate::canonical`] during
//! initial population and by the struct/tuple layout optimization pass.
//!
//! # Submodules
//!
//! - [`struct_layout`] — field reordering for structs
//! - [`tuple_layout`] — field reordering for tuples
//! - [`niche`] — niche analysis for enum optimization
//! - [`tagged_ptr`] — tagged pointer analysis for enum optimization

pub(crate) mod niche;
pub(crate) mod struct_layout;
pub(crate) mod tagged_ptr;
pub(crate) mod tuple_layout;

#[cfg(test)]
mod tests;

use crate::enum_repr::VariantRepr;
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::{FieldRepr, TupleRepr};

/// Whether a repr is trivial (no RC operations needed).
///
/// Recursively checks compound types: a struct/tuple is trivial if all
/// fields are trivial; an enum is trivial if all variant fields are trivial.
///
/// This is the **single canonical definition** of triviality at the
/// `MachineRepr` level — used by both `canonical::populate_canonical()`
/// and `ReprPlan::is_trivial()`.
pub(crate) fn is_trivial_repr(repr: &MachineRepr) -> bool {
    match repr {
        MachineRepr::Int { .. }
        | MachineRepr::Float { .. }
        | MachineRepr::Bool
        | MachineRepr::Char
        | MachineRepr::Byte
        | MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Ordering
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::Range
        | MachineRepr::UnmanagedPtr => true,
        MachineRepr::Struct(s) => s.trivial,
        MachineRepr::Tuple(t) => t.trivial,
        MachineRepr::Enum(e) => e
            .variants
            .iter()
            .all(|v| v.fields.iter().all(is_trivial_repr)),
        MachineRepr::FatPointer(_)
        | MachineRepr::Closure(_)
        | MachineRepr::RcPointer(_)
        | MachineRepr::OpaquePtr
        | MachineRepr::StackPromoted { .. } => false,
    }
}

/// Size of a repr when used as a field in an aggregate (struct/tuple/enum).
///
/// Unit and Never are zero-sized in aggregates — they contribute no bytes
/// to the containing struct/tuple/enum layout. For standalone by-value
/// lowering (LLVM i64 materialization), use [`repr_size`] instead.
pub(crate) fn field_size(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Unit | MachineRepr::Never => 0,
        other => repr_size(other),
    }
}

/// Alignment of a repr when used as a field in an aggregate.
///
/// Unit and Never have alignment 1 (minimum) in aggregates — they don't
/// inflate the containing type's alignment requirement.
pub(crate) fn field_align(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Unit | MachineRepr::Never => 1,
        other => repr_align(other),
    }
}

/// ABI size of a repr in bytes (for by-value lowering).
///
/// Unit and Never are 8 bytes here (LLVM i64 representation) because
/// they must be materializable as standalone values. For aggregate field
/// sizing, use [`field_size`] instead.
pub(crate) fn repr_size(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Int { width, .. } => width.size_bytes(),
        MachineRepr::Float { width } => width.size_bytes(),
        MachineRepr::Bool | MachineRepr::Byte | MachineRepr::Ordering => 1,
        MachineRepr::Char => 4,
        MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::RcPointer(_)
        | MachineRepr::OpaquePtr
        | MachineRepr::UnmanagedPtr => 8,
        MachineRepr::Struct(s) => s.size,
        MachineRepr::Enum(e) => e.size,
        MachineRepr::Tuple(t) => t.size,
        MachineRepr::FatPointer(_) => 24, // {i64, i64, ptr}
        MachineRepr::Closure(_) => 16,    // {ptr fn, ptr env}
        MachineRepr::Range => 32,         // {i64, i64, i64, i64}
        MachineRepr::StackPromoted { inner, .. } => repr_size(inner),
    }
}

/// ABI alignment of a repr in bytes.
pub(crate) fn repr_align(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Int { width, .. } => width.alignment(),
        MachineRepr::Float { width } => width.alignment(),
        MachineRepr::Bool | MachineRepr::Byte | MachineRepr::Ordering => 1,
        MachineRepr::Char => 4,
        MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::FatPointer(_)
        | MachineRepr::Closure(_)
        | MachineRepr::Range
        | MachineRepr::RcPointer(_)
        | MachineRepr::OpaquePtr
        | MachineRepr::UnmanagedPtr => 8,
        MachineRepr::Struct(s) => s.align,
        MachineRepr::Enum(e) => e.align,
        MachineRepr::Tuple(t) => t.align,
        MachineRepr::StackPromoted { inner, .. } => repr_align(inner),
    }
}

/// Round `size` up to the next multiple of `align`.
pub(crate) fn round_up(size: u32, align: u32) -> u32 {
    if align == 0 {
        return size;
    }
    let remainder = size % align;
    if remainder == 0 {
        size
    } else {
        size + align - remainder
    }
}

/// Compute ABI-correct layout (size, alignment) for named/tuple fields.
///
/// Uses [`field_size`]/[`field_align`] so that zero-sized types (Unit, Never)
/// don't inflate the containing aggregate.
pub(crate) fn compute_field_layout(fields: &[FieldRepr]) -> (u32, u32) {
    let align = fields
        .iter()
        .map(|f| field_align(&f.repr))
        .max()
        .unwrap_or(1);
    let mut offset = 0u32;
    for f in fields {
        let fa = field_align(&f.repr);
        offset = round_up(offset, fa);
        offset += field_size(&f.repr);
    }
    (round_up(offset, align), align)
}

/// Compute size and alignment for a tagless (single-variant) enum.
///
/// Unit newtypes get minimal size (1, 1) for LLVM struct compatibility.
/// Non-unit variants use their payload's size and alignment.
#[must_use]
pub(crate) fn compute_tagless_enum_layout(variant: &VariantRepr) -> (u32, u32) {
    if variant.size == 0 {
        (1, 1) // Unit newtype: minimal size for LLVM struct
    } else {
        (variant.size, variant.alignment.max(1))
    }
}

/// Compute layout for an enum variant's payload using `[M x i64]` slot packing.
///
/// Each field occupies at least one full i64 slot (8 bytes), regardless of its
/// natural alignment. This matches `resolve_enum()` in `ori_llvm` and
/// `enum_payload_size()` / `pool_type_store_size()` in `ori_arc`.
pub(crate) fn compute_enum_payload_layout(fields: &[MachineRepr]) -> (u32, u32) {
    let payload: u32 = fields
        .iter()
        .map(|f| {
            let size = field_size(f);
            // Round up to 8-byte i64 slot boundary
            size.div_ceil(8) * 8
        })
        .sum();
    let align = if payload > 0 { 8 } else { 1 };
    (payload, align)
}

/// Reorder fields by descending alignment, then descending size.
///
/// Returns `(reordered_fields, total_size, max_alignment)`. The returned
/// fields have their `offset` set to the byte offset in the reordered layout.
///
/// This is the shared core of both struct and tuple layout optimization.
/// Struct-specific concerns (padding diagnostics, `#repr` attributes) are
/// handled by the callers in [`struct_layout`] and [`tuple_layout`].
pub(crate) fn reorder_fields(fields: &[FieldRepr]) -> (Vec<FieldRepr>, u32, u32) {
    if fields.is_empty() {
        return (vec![], 0, 1);
    }

    // Build (current_position, size, align) tuples for sorting.
    let mut indexed: Vec<(usize, u32, u32)> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let size = field_size(&f.repr);
            let align = field_align(&f.repr);
            (i, size, align)
        })
        .collect();

    // Stable sort: fields with equal alignment AND size preserve
    // declaration order — deterministic, reproducible layout.
    indexed.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));

    // Compute offsets in sorted order.
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut result = Vec::with_capacity(fields.len());

    for &(src_idx, size, align) in &indexed {
        offset = round_up(offset, align);
        let mut field = fields[src_idx].clone();
        field.offset = offset;
        result.push(field);
        offset += size;
        max_align = max_align.max(align);
    }

    let total_size = round_up(offset, max_align);
    (result, total_size, max_align)
}

/// Compute explicit-tag enum layout from tag width and variant dimensions.
///
/// Returns `(total_size, alignment)` for an enum with an explicit discriminant
/// tag. Used by `canonical_enum`, `default_option_repr`, and `default_result_repr`
/// to avoid duplicating the tag-padding logic.
///
/// Layout rule: all-unit enums (no payload) use the tag width as both size
/// and alignment. Non-unit enums pad the tag to an 8-byte slot and append
/// the payload aligned to `max(variant_align, 8)` (LLVM `[M x i64]` convention).
pub(crate) fn compute_explicit_tag_layout(
    tag_width: IntWidth,
    max_payload: u32,
    max_variant_align: u32,
) -> (u32, u32) {
    let tag_size = tag_width.size_bytes();
    if max_payload == 0 {
        (tag_size, tag_size)
    } else {
        let payload_align = max_variant_align.max(8);
        (8 + round_up(max_payload, payload_align), payload_align)
    }
}

impl TupleRepr {
    /// Convert tuple fields into a `MachineRepr::Tuple` with ABI-correct layout.
    pub(crate) fn to_machine_repr(fields: Vec<FieldRepr>, trivial: bool) -> MachineRepr {
        let (size, align) = compute_field_layout(&fields);
        MachineRepr::Tuple(TupleRepr {
            elements: fields,
            size,
            align,
            trivial,
        })
    }
}
