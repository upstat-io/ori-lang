//! Struct field reordering for optimal alignment and minimal padding.
//!
//! Implements the field reordering algorithm (§06.1) and ABI-stable
//! layout variants (§06.3: `#repr("c")`, `#repr("packed")`,
//! `#repr("transparent")`, `#repr("aligned", N)`).

use crate::layout::{field_align, field_size, round_up};
use crate::plan::ReprAttribute;
use crate::struct_repr::StructRepr;

/// Reorder struct fields for optimal alignment and minimal padding.
///
/// Reads the existing `StructRepr` (already populated by `canonical_struct()`
/// with narrowed field reprs from §04/§05), reorders fields by descending
/// alignment then descending size, computes byte offsets, and returns the
/// updated `StructRepr`.
///
/// Skips types with `#repr("c")`, `#repr("packed")`, or
/// `#repr("transparent")` attributes — those have user-controlled layout.
pub(crate) fn optimize_struct_layout(
    struct_repr: &StructRepr,
    repr_attr: Option<&ReprAttribute>,
) -> StructRepr {
    match repr_attr {
        Some(ReprAttribute::C | ReprAttribute::CAligned(_)) => {
            compute_c_layout(struct_repr, repr_attr)
        }
        Some(ReprAttribute::Packed) => compute_packed_layout(struct_repr),
        Some(ReprAttribute::Transparent) => compute_transparent_layout(struct_repr),
        Some(ReprAttribute::Aligned(n)) => {
            let mut result = reorder_and_layout(struct_repr);
            result.align = result.align.max(*n);
            result.size = round_up(result.size, result.align);
            result
        }
        Some(ReprAttribute::Default) | None => reorder_and_layout(struct_repr),
    }
}

/// Core reordering: sort fields by descending alignment, then descending size.
fn reorder_and_layout(struct_repr: &StructRepr) -> StructRepr {
    if struct_repr.fields.is_empty() {
        return StructRepr {
            fields: vec![],
            size: 0,
            align: 1,
            trivial: struct_repr.trivial,
        };
    }

    // Build (current_position, size, align) tuples for sorting.
    let mut indexed: Vec<(usize, u32, u32)> = struct_repr
        .fields
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
    let mut fields = Vec::with_capacity(struct_repr.fields.len());

    for &(src_idx, size, align) in &indexed {
        offset = round_up(offset, align);
        let mut field = struct_repr.fields[src_idx].clone();
        field.offset = offset;
        fields.push(field);
        offset += size;
        max_align = max_align.max(align);
    }

    let total_size = round_up(offset, max_align);

    StructRepr {
        fields,
        size: total_size,
        align: max_align,
        trivial: struct_repr.trivial,
    }
}

/// C layout: fields in declaration order with platform alignment.
fn compute_c_layout(struct_repr: &StructRepr, repr_attr: Option<&ReprAttribute>) -> StructRepr {
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut fields = Vec::with_capacity(struct_repr.fields.len());

    for f in &struct_repr.fields {
        let align = field_align(&f.repr);
        let size = field_size(&f.repr);
        offset = round_up(offset, align);
        let mut field = f.clone();
        field.offset = offset;
        fields.push(field);
        offset += size;
        max_align = max_align.max(align);
    }

    // Apply forced alignment from CAligned(N).
    if let Some(ReprAttribute::CAligned(n)) = repr_attr {
        max_align = max_align.max(*n);
    }

    let total_size = round_up(offset, max_align);

    StructRepr {
        fields,
        size: total_size,
        align: max_align,
        trivial: struct_repr.trivial,
    }
}

/// Packed layout: no padding, alignment = 1.
fn compute_packed_layout(struct_repr: &StructRepr) -> StructRepr {
    let mut offset = 0u32;
    let mut fields = Vec::with_capacity(struct_repr.fields.len());

    for f in &struct_repr.fields {
        let size = field_size(&f.repr);
        let mut field = f.clone();
        field.offset = offset;
        fields.push(field);
        offset += size;
    }

    StructRepr {
        fields,
        size: offset,
        align: 1,
        trivial: struct_repr.trivial,
    }
}

/// Transparent layout: same layout as the single non-ZST field.
///
/// # Panics
///
/// Panics (`debug_assert`) if the struct has != 1 non-zero-sized field.
/// Validation should catch this earlier in `ori_types`, but we guard
/// here as defense-in-depth.
fn compute_transparent_layout(struct_repr: &StructRepr) -> StructRepr {
    let non_zst: Vec<_> = struct_repr
        .fields
        .iter()
        .filter(|f| field_size(&f.repr) > 0)
        .collect();

    debug_assert!(
        non_zst.len() == 1,
        "#repr(\"transparent\") requires exactly 1 non-ZST field, got {}",
        non_zst.len()
    );

    if non_zst.len() == 1 {
        let inner = non_zst[0];
        let size = field_size(&inner.repr);
        let align = field_align(&inner.repr);

        let mut fields = Vec::with_capacity(struct_repr.fields.len());
        for f in &struct_repr.fields {
            let mut field = f.clone();
            if field_size(&f.repr) > 0 {
                field.offset = 0;
            } else {
                field.offset = 0; // ZST fields get offset 0 too
            }
            fields.push(field);
        }

        StructRepr {
            fields,
            size,
            align,
            trivial: struct_repr.trivial,
        }
    } else {
        // Fallback: treat as default layout (shouldn't happen if validation is correct).
        reorder_and_layout(struct_repr)
    }
}
