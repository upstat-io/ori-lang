//! Tests for layout computation and field reordering.

use ori_ir::Name;

use crate::layout::struct_layout::optimize_struct_layout;
use crate::layout::tuple_layout::optimize_tuple_layout;
use crate::plan::ReprAttribute;
use crate::repr::{IntWidth, MachineRepr};
use crate::struct_repr::{FieldRepr, StructRepr, TupleRepr};

// Helper: create a FieldRepr with the given index and repr.
// Name is a synthetic interned ID — field names don't affect layout.
fn field(index: u32, repr: MachineRepr) -> FieldRepr {
    FieldRepr {
        name: Name::from_raw(index),
        original_index: index,
        offset: 0,
        repr,
    }
}

fn bool_repr() -> MachineRepr {
    MachineRepr::Bool
}

fn int_repr() -> MachineRepr {
    MachineRepr::Int {
        width: IntWidth::I64,
        signed: true,
    }
}

fn byte_repr() -> MachineRepr {
    MachineRepr::Byte
}

fn i16_repr() -> MachineRepr {
    MachineRepr::Int {
        width: IntWidth::I16,
        signed: true,
    }
}

fn f32_repr() -> MachineRepr {
    MachineRepr::Float {
        width: crate::repr::FloatWidth::F32,
    }
}

fn float_repr() -> MachineRepr {
    MachineRepr::Float {
        width: crate::repr::FloatWidth::F64,
    }
}

fn make_struct(fields: Vec<FieldRepr>) -> StructRepr {
    StructRepr {
        fields,
        size: 0,
        align: 1,
        trivial: true,
    }
}

fn make_tuple(elements: Vec<FieldRepr>) -> TupleRepr {
    TupleRepr {
        elements,
        size: 0,
        align: 1,
        trivial: true,
    }
}

// ─── StructRepr helper method tests ───────────────────────────

#[test]
fn test_field_by_original_found() {
    let s = StructRepr {
        fields: vec![
            field(2, int_repr()),
            field(0, bool_repr()),
            field(1, byte_repr()),
        ],
        size: 16,
        align: 8,
        trivial: true,
    };
    let f = s.field_by_original(0);
    assert!(f.is_some(), "field 0 should exist");
    assert_eq!(f.map(|f| f.original_index), Some(0));
}

#[test]
fn test_field_by_original_not_found() {
    let s = make_struct(vec![field(0, int_repr())]);
    assert!(s.field_by_original(5).is_none());
}

#[test]
fn test_memory_index_identity() {
    // Before reordering, memory_index(i) == i.
    let s = make_struct(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, byte_repr()),
    ]);
    assert_eq!(s.memory_index(0), Some(0));
    assert_eq!(s.memory_index(1), Some(1));
    assert_eq!(s.memory_index(2), Some(2));
}

#[test]
fn test_memory_index_out_of_range() {
    let s = make_struct(vec![field(0, int_repr())]);
    assert_eq!(s.memory_index(1), None);
}

#[test]
fn test_memory_index_empty_struct() {
    let s = make_struct(vec![]);
    assert_eq!(s.memory_index(0), None);
}

#[test]
fn test_is_reordered_identity() {
    let s = make_struct(vec![field(0, bool_repr()), field(1, int_repr())]);
    assert!(!s.is_reordered());
}

#[test]
fn test_is_reordered_swapped() {
    let s = StructRepr {
        fields: vec![field(1, int_repr()), field(0, bool_repr())],
        size: 16,
        align: 8,
        trivial: true,
    };
    assert!(s.is_reordered());
}

// ─── TupleRepr helper method tests ────────────────────────────

#[test]
fn test_tuple_memory_index_identity() {
    let t = make_tuple(vec![field(0, bool_repr()), field(1, int_repr())]);
    assert_eq!(t.memory_index(0), Some(0));
    assert_eq!(t.memory_index(1), Some(1));
}

#[test]
fn test_tuple_is_reordered_identity() {
    let t = make_tuple(vec![field(0, bool_repr()), field(1, int_repr())]);
    assert!(!t.is_reordered());
}

// ─── Struct reordering tests ──────────────────────────────────

#[test]
fn test_reorder_bool_int_bool() {
    // { a: bool, b: int, c: bool } → int first, then bools
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 16);
    assert_eq!(result.align, 8);
    assert_eq!(result.fields.len(), 3);
    // int (align 8) should be first
    assert_eq!(result.fields[0].original_index, 1);
    assert_eq!(result.fields[0].offset, 0);
    // bools follow
    assert_eq!(result.fields[1].original_index, 0);
    assert_eq!(result.fields[1].offset, 8);
    assert_eq!(result.fields[2].original_index, 2);
    assert_eq!(result.fields[2].offset, 9);
}

#[test]
fn test_reorder_already_optimal() {
    // { x: int, y: int } → no change needed
    let input = make_struct(vec![field(0, int_repr()), field(1, int_repr())]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 16);
    assert_eq!(result.align, 8);
    assert_eq!(result.fields[0].original_index, 0);
    assert_eq!(result.fields[1].original_index, 1);
}

#[test]
fn test_reorder_bytes_and_int() {
    // { a: byte, b: byte, c: byte, d: byte, e: int }
    // → int first at offset 0, bytes at 8-11, padding 12-15
    let input = make_struct(vec![
        field(0, byte_repr()),
        field(1, byte_repr()),
        field(2, byte_repr()),
        field(3, byte_repr()),
        field(4, int_repr()),
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 16);
    assert_eq!(result.align, 8);
    assert_eq!(result.fields[0].original_index, 4); // int first
    assert_eq!(result.fields[0].offset, 0);
    assert_eq!(result.fields[1].offset, 8);
    assert_eq!(result.fields[2].offset, 9);
    assert_eq!(result.fields[3].offset, 10);
    assert_eq!(result.fields[4].offset, 11);
}

#[test]
fn test_reorder_empty_struct() {
    let input = make_struct(vec![]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 0);
    assert_eq!(result.align, 1);
    assert!(result.fields.is_empty());
}

#[test]
fn test_reorder_single_field() {
    let input = make_struct(vec![field(0, int_repr())]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 8);
    assert_eq!(result.align, 8);
    assert_eq!(result.fields[0].original_index, 0);
    assert_eq!(result.fields[0].offset, 0);
}

#[test]
fn test_reorder_zst_field() {
    // { a: Unit, b: int } → int at offset 0, Unit at offset 8 (0 bytes)
    let input = make_struct(vec![field(0, MachineRepr::Unit), field(1, int_repr())]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 8);
    assert_eq!(result.align, 8);
    // int comes first (align 8 > 1)
    assert_eq!(result.fields[0].original_index, 1);
}

#[test]
fn test_reorder_mixed_widths() {
    // { a: bool, b: float, c: byte, d: int }
    // → float(8,align8), int(8,align8), bool(1,align1), byte(1,align1)
    // Stable sort: float before int (both align 8, size 8 — float at input pos 1, int at pos 3)
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, float_repr()),
        field(2, byte_repr()),
        field(3, int_repr()),
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 24);
    assert_eq!(result.align, 8);
    // float and int tied on alignment+size, stable sort preserves input order
    assert_eq!(result.fields[0].original_index, 1); // float
    assert_eq!(result.fields[1].original_index, 3); // int
    assert_eq!(result.fields[2].original_index, 0); // bool
    assert_eq!(result.fields[3].original_index, 2); // byte
}

#[test]
fn test_reorder_narrowed_fields() {
    // { a: bool(1,align1), b: i16(2,align2), c: f32(4,align4) }
    // → f32(align4) first, then i16(align2), then bool(align1)
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, i16_repr()),
        field(2, f32_repr()),
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 8);
    assert_eq!(result.align, 4);
    assert_eq!(result.fields[0].original_index, 2); // f32
    assert_eq!(result.fields[0].offset, 0);
    assert_eq!(result.fields[1].original_index, 1); // i16
    assert_eq!(result.fields[1].offset, 4);
    assert_eq!(result.fields[2].original_index, 0); // bool
    assert_eq!(result.fields[2].offset, 6);
}

#[test]
fn test_reorder_stable_sort_preserves_order() {
    // All same type (int): stable sort preserves declaration order.
    let input = make_struct(vec![
        field(0, int_repr()),
        field(1, int_repr()),
        field(2, int_repr()),
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.fields[0].original_index, 0);
    assert_eq!(result.fields[1].original_index, 1);
    assert_eq!(result.fields[2].original_index, 2);
}

#[test]
fn test_memory_index_after_reorder() {
    // After reordering { a: bool, b: int }, memory_index(0) == 1 (bool moved)
    let input = make_struct(vec![field(0, bool_repr()), field(1, int_repr())]);
    let result = optimize_struct_layout(&input, None);

    // int is at memory position 0, bool at position 1
    assert_eq!(result.memory_index(1), Some(0)); // int → pos 0
    assert_eq!(result.memory_index(0), Some(1)); // bool → pos 1
}

// ─── ABI-stable layout tests ─────────────────────────────────

#[test]
fn test_c_layout_preserves_order() {
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::C));

    assert_eq!(result.size, 24);
    assert_eq!(result.align, 8);
    assert_eq!(result.fields[0].original_index, 0);
    assert_eq!(result.fields[1].original_index, 1);
    assert_eq!(result.fields[2].original_index, 2);
    // C layout: bool at 0, pad 1-7, int at 8, bool at 16, pad 17-23
    assert_eq!(result.fields[0].offset, 0);
    assert_eq!(result.fields[1].offset, 8);
    assert_eq!(result.fields[2].offset, 16);
}

#[test]
fn test_c_aligned_layout() {
    let input = make_struct(vec![field(0, bool_repr()), field(1, int_repr())]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::CAligned(16)));

    // C layout + forced alignment 16
    assert_eq!(result.align, 16);
    assert_eq!(result.size, 16); // round_up(9, 16) = 16
    assert_eq!(result.fields[0].original_index, 0); // declaration order
    assert_eq!(result.fields[1].original_index, 1);
}

#[test]
fn test_packed_no_padding() {
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::Packed));

    assert_eq!(result.size, 10); // 1 + 8 + 1, no padding
    assert_eq!(result.align, 1);
    assert_eq!(result.fields[0].offset, 0);
    assert_eq!(result.fields[1].offset, 1);
    assert_eq!(result.fields[2].offset, 9);
}

#[test]
fn test_transparent_single_field() {
    let input = make_struct(vec![field(0, int_repr())]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::Transparent));

    assert_eq!(result.size, 8);
    assert_eq!(result.align, 8);
}

#[test]
fn test_transparent_with_zst() {
    let input = make_struct(vec![field(0, int_repr()), field(1, MachineRepr::Unit)]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::Transparent));

    assert_eq!(result.size, 8); // ZST ignored
    assert_eq!(result.align, 8);
}

#[test]
fn test_aligned_increases_alignment() {
    let input = make_struct(vec![field(0, int_repr())]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::Aligned(16)));

    assert_eq!(result.align, 16);
    assert_eq!(result.size, 16); // 8 bytes data + 8 padding to align 16
}

#[test]
fn test_aligned_does_not_decrease_alignment() {
    // int has align 8, Aligned(4) should not decrease it.
    let input = make_struct(vec![field(0, int_repr())]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::Aligned(4)));

    assert_eq!(result.align, 8); // max(computed=8, requested=4) = 8
    assert_eq!(result.size, 8);
}

#[test]
fn test_default_reorders() {
    let input = make_struct(vec![field(0, bool_repr()), field(1, int_repr())]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::Default));

    // Default = reorder
    assert_eq!(result.fields[0].original_index, 1); // int first
    assert_eq!(result.size, 16);
}

// ─── Negative pin tests ──────────────────────────────────────

#[test]
fn test_negative_pin_reordered_not_24() {
    // { a: bool, b: int, c: bool } must NOT be 24 (unoptimized size)
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_struct_layout(&input, None);
    assert_ne!(result.size, 24);
}

#[test]
fn test_negative_pin_c_layout_not_16() {
    // #repr("c") { a: bool, b: int, c: bool } must NOT be 16 (optimized size)
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_struct_layout(&input, Some(&ReprAttribute::C));
    assert_ne!(result.size, 16);
}

// ─── Tuple reordering tests ──────────────────────────────────

#[test]
fn test_tuple_reorder_bool_int_bool() {
    let input = make_tuple(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_tuple_layout(&input);

    assert_eq!(result.size, 16);
    assert_eq!(result.align, 8);
    assert_eq!(result.elements[0].original_index, 1); // int first
}

#[test]
fn test_tuple_preserves_original_index() {
    let input = make_tuple(vec![field(0, bool_repr()), field(1, int_repr())]);
    let result = optimize_tuple_layout(&input);

    // After reorder: elements[0] is int with original_index=1
    assert_eq!(result.elements[0].original_index, 1);
}

#[test]
fn test_tuple_memory_index_after_reorder() {
    let input = make_tuple(vec![field(0, bool_repr()), field(1, int_repr())]);
    let result = optimize_tuple_layout(&input);

    assert_eq!(result.memory_index(0), Some(1)); // bool → pos 1
    assert_eq!(result.memory_index(1), Some(0)); // int → pos 0
}

#[test]
fn test_tuple_single_element() {
    let input = make_tuple(vec![field(0, int_repr())]);
    let result = optimize_tuple_layout(&input);

    assert_eq!(result.size, 8);
    assert_eq!(result.elements[0].original_index, 0);
}

#[test]
fn test_tuple_all_same_type() {
    // Stable sort preserves order when all elements have same alignment+size.
    let input = make_tuple(vec![
        field(0, int_repr()),
        field(1, int_repr()),
        field(2, int_repr()),
    ]);
    let result = optimize_tuple_layout(&input);

    assert_eq!(result.elements[0].original_index, 0);
    assert_eq!(result.elements[1].original_index, 1);
    assert_eq!(result.elements[2].original_index, 2);
}

// ─── Semantic pin: the canonical §06 proof ───────────────────

#[test]
fn test_semantic_pin_struct_reorder() {
    // { a: bool, b: int, c: bool, d: byte } →
    // int(8) at offset 0, bool(1) at 8, bool(1) at 9, byte(1) at 10
    // Total: round_up(11, 8) = 16 (not 32 with naive layout)
    let input = make_struct(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
        field(3, byte_repr()),
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(
        result.size, 16,
        "§06 semantic pin: struct must be 16 bytes, not 32"
    );
    assert_eq!(result.fields[0].original_index, 1, "int should be first");
    assert!(
        matches!(
            result.fields[0].repr,
            MachineRepr::Int {
                width: IntWidth::I64,
                ..
            }
        ),
        "first field should be Int(I64)"
    );
}
