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

use crate::struct_repr::TupleRepr;

/// Reorder tuple elements for optimal alignment and minimal padding.
///
/// Same algorithm as struct reordering (delegates to [`super::reorder_fields`])
/// but operates on `TupleRepr.elements`. `original_index` is the tuple
/// position (0, 1, 2, ...) for codegen remapping of `.0`, `.1`, `.2` accesses.
pub(crate) fn optimize_tuple_layout(tuple_repr: &TupleRepr) -> TupleRepr {
    let (elements, size, align) = super::reorder_fields(&tuple_repr.elements);

    TupleRepr {
        elements,
        size,
        align,
        trivial: tuple_repr.trivial,
    }
}
