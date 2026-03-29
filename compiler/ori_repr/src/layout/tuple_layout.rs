//! Tuple field reordering for optimal alignment and minimal padding.
//!
//! Tuples are anonymous structs — the same reordering algorithm applies.
//! No `#repr` attributes apply to tuples (they are always reorderable).

use crate::layout::{field_align, field_size, round_up};
use crate::struct_repr::TupleRepr;

/// Reorder tuple elements for optimal alignment and minimal padding.
///
/// Same algorithm as struct reordering but operates on `TupleRepr.elements`.
/// `original_index` is the tuple position (0, 1, 2, ...) for codegen
/// remapping of `.0`, `.1`, `.2` accesses.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "activated when compute_struct_layouts enables reordering"
    )
)]
pub(crate) fn optimize_tuple_layout(tuple_repr: &TupleRepr) -> TupleRepr {
    if tuple_repr.elements.is_empty() {
        return TupleRepr {
            elements: vec![],
            size: 0,
            align: 1,
            trivial: tuple_repr.trivial,
        };
    }

    // Build (current_position, size, align) tuples for sorting.
    let mut indexed: Vec<(usize, u32, u32)> = tuple_repr
        .elements
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let size = field_size(&f.repr);
            let align = field_align(&f.repr);
            (i, size, align)
        })
        .collect();

    // Stable sort: equal alignment+size preserves declaration order.
    indexed.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));

    // Compute offsets in sorted order.
    let mut offset = 0u32;
    let mut max_align = 1u32;
    let mut elements = Vec::with_capacity(tuple_repr.elements.len());

    for &(src_idx, size, align) in &indexed {
        offset = round_up(offset, align);
        let mut elem = tuple_repr.elements[src_idx].clone();
        elem.offset = offset;
        elements.push(elem);
        offset += size;
        max_align = max_align.max(align);
    }

    let total_size = round_up(offset, max_align);

    TupleRepr {
        elements,
        size: total_size,
        align: max_align,
        trivial: tuple_repr.trivial,
    }
}
