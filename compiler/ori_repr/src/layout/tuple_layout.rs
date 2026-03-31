//! Tuple field reordering for optimal alignment and minimal padding.
//!
//! Tuples are anonymous structs — the same reordering algorithm applies.
//! No `#repr` attributes apply to tuples (they are always reorderable).
//!
//! Only 3+ element tuples are reordered. 2-element tuples are skipped because:
//! - All runtime-boundary tuples are 2-element (`next_map`, `next_zipped`,
//!   `next_enumerated` write fields at hardcoded byte offsets).
//! - For types where size is a multiple of alignment (all Ori types),
//!   2-element tuple total size is identical regardless of field order.

use crate::layout::{field_align, field_size, round_up};
use crate::struct_repr::TupleRepr;

/// Reorder tuple elements for optimal alignment and minimal padding.
///
/// Same algorithm as struct reordering but operates on `TupleRepr.elements`.
/// `original_index` is the tuple position (0, 1, 2, ...) for codegen
/// remapping of `.0`, `.1`, `.2` accesses.
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
