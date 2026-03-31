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

// ─── Mixed-field (non-scalar) struct reordering ─────────────

fn fat_ptr_str_repr() -> MachineRepr {
    MachineRepr::FatPointer(crate::struct_repr::FatRepr::Str)
}

fn rc_ptr_repr() -> MachineRepr {
    MachineRepr::RcPointer(crate::struct_repr::RcRepr {
        rc_width: IntWidth::I64,
        atomic: false,
        inner: Box::new(MachineRepr::Unit),
        stack_promotable: false,
    })
}

fn closure_repr() -> MachineRepr {
    MachineRepr::Closure(crate::struct_repr::ClosureRepr {
        params: vec![],
        ret: Box::new(MachineRepr::Unit),
    })
}

#[test]
fn test_reorder_mixed_bool_str_int_bool() {
    // { flag: bool, name: str, count: int, active: bool }
    // str is FatPointer(24 bytes, align 8), int is 8, bool is 1
    // Declaration: bool(1) pad 7 + str(24) + int(8) + bool(1) pad 7 = 48
    // Reordered (desc align, desc size): str(24) + int(8) + bool(1) + bool(1) pad 6 = 40
    let input = make_struct(vec![
        field(0, bool_repr()),        // 1 byte, align 1
        field(1, fat_ptr_str_repr()), // 24 bytes, align 8
        field(2, int_repr()),         // 8 bytes, align 8
        field(3, bool_repr()),        // 1 byte, align 1
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 40, "mixed struct should be 40 bytes, not 48");
    assert_eq!(result.align, 8);
    // str (24 bytes) first, then int (8 bytes), then bool, bool
    assert_eq!(
        result.fields[0].original_index, 1,
        "str should be first (largest at align 8)"
    );
    assert_eq!(result.fields[1].original_index, 2, "int should be second");
    assert_eq!(result.fields[0].offset, 0);
    assert_eq!(result.fields[1].offset, 24);
    assert_eq!(result.fields[2].offset, 32);
    assert_eq!(result.fields[3].offset, 33);
}

#[test]
fn test_reorder_mixed_rc_and_scalars() {
    // { a: bool, b: RcPointer, c: int, d: byte }
    // RcPointer = 8 bytes, same as int. Both at align 8.
    // Reordered: RcPointer(8), int(8), bool(1), byte(1) pad 6 = 24
    let input = StructRepr {
        fields: vec![
            field(0, bool_repr()),   // 1, align 1
            field(1, rc_ptr_repr()), // 8, align 8
            field(2, int_repr()),    // 8, align 8
            field(3, byte_repr()),   // 1, align 1
        ],
        size: 0,
        align: 1,
        trivial: false, // non-trivial: contains RC field
    };
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 24);
    assert_eq!(result.align, 8);
    // RcPointer and int are same size+align — stable sort preserves order
    assert_eq!(result.fields[0].original_index, 1, "rc_ptr first");
    assert_eq!(result.fields[1].original_index, 2, "int second");
    assert_eq!(result.fields[2].original_index, 0, "bool third");
    assert_eq!(result.fields[3].original_index, 3, "byte fourth");
    assert!(!result.trivial, "struct with RC field is non-trivial");
}

#[test]
fn test_reorder_mixed_closure_field() {
    // { a: bool, b: Closure(16), c: int }
    // Reordered: Closure(16), int(8), bool(1) pad 7 = 32
    let input = make_struct(vec![
        field(0, bool_repr()),    // 1, align 1
        field(1, closure_repr()), // 16, align 8
        field(2, int_repr()),     // 8, align 8
    ]);
    let result = optimize_struct_layout(&input, None);

    assert_eq!(result.size, 32);
    assert_eq!(
        result.fields[0].original_index, 1,
        "closure first (largest at align 8)"
    );
    assert_eq!(result.fields[1].original_index, 2, "int second");
    assert_eq!(result.fields[2].original_index, 0, "bool last");
}

#[test]
fn test_reorder_mixed_preserves_original_index() {
    // Verify original_index is correctly preserved through reordering
    let input = make_struct(vec![
        field(0, byte_repr()),        // 1, align 1
        field(1, fat_ptr_str_repr()), // 24, align 8
        field(2, bool_repr()),        // 1, align 1
    ]);
    let result = optimize_struct_layout(&input, None);

    // str should be first in memory, but original_index should be 1
    assert_eq!(result.fields[0].original_index, 1);
    assert_eq!(result.memory_index(1), Some(0)); // str decl idx 1 → mem pos 0
    assert_eq!(result.memory_index(0), Some(1)); // byte decl idx 0 → mem pos 1
    assert_eq!(result.memory_index(2), Some(2)); // bool decl idx 2 → mem pos 2
}

#[test]
fn test_reorder_mixed_nontrivial_flag() {
    // Struct with non-scalar fields should NOT be marked trivial
    let input = StructRepr {
        fields: vec![field(0, int_repr()), field(1, fat_ptr_str_repr())],
        size: 0,
        align: 1,
        trivial: false, // non-trivial input
    };
    let result = optimize_struct_layout(&input, None);

    assert!(!result.trivial, "non-trivial flag must be preserved");
}

#[test]
fn test_reorder_mixed_tuple() {
    // (bool, str, int) — same layout optimization as struct
    let input = make_tuple(vec![
        field(0, bool_repr()),
        field(1, fat_ptr_str_repr()),
        field(2, int_repr()),
    ]);
    let result = optimize_tuple_layout(&input);

    assert_eq!(result.size, 40);
    assert_eq!(result.elements[0].original_index, 1, "str first");
    assert_eq!(result.elements[1].original_index, 2, "int second");
    assert_eq!(result.elements[2].original_index, 0, "bool last");
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

// ─── Tuple pipeline activation tests ──────────────────────────

#[test]
fn test_two_element_tuple_size_invariant() {
    // For 2-element tuples where field sizes are multiples of alignment
    // (all Ori types), reordering never changes total size.
    // This is why the pipeline safely skips 2-element tuples.
    let pairs: Vec<(MachineRepr, MachineRepr)> = vec![
        (bool_repr(), int_repr()),
        (byte_repr(), int_repr()),
        (int_repr(), bool_repr()),
        (MachineRepr::Char, int_repr()),
        (bool_repr(), bool_repr()),
    ];
    for (a, b) in &pairs {
        let forward = make_tuple(vec![field(0, a.clone()), field(1, b.clone())]);
        let forward_opt = optimize_tuple_layout(&forward);
        let reverse = make_tuple(vec![field(0, b.clone()), field(1, a.clone())]);
        let reverse_opt = optimize_tuple_layout(&reverse);
        assert_eq!(
            forward_opt.size, reverse_opt.size,
            "2-element tuple: size must be identical regardless of field order"
        );
    }
}

#[test]
fn test_three_element_tuple_reorder_saves_space() {
    // (bool, int, bool): optimize_tuple_layout computes the reordered layout.
    // Declaration order would be 24 bytes; reordered is 16.
    // Semantic pin: 3+ element tuples benefit from reordering.
    let input = make_tuple(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_tuple_layout(&input);
    assert_eq!(
        result.size, 16,
        "§06.4 semantic pin: 3-element tuple must be 16 bytes, not 24"
    );
    // int moved to front
    assert_eq!(result.elements[0].original_index, 1);
    assert_eq!(result.elements[1].original_index, 0);
    assert_eq!(result.elements[2].original_index, 2);
}

#[test]
fn test_four_element_tuple_reorder() {
    // (bool, int, byte, float) → reordered: int(8), float(8), bool(1), byte(1)
    let input = make_tuple(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, byte_repr()),
        field(3, float_repr()),
    ]);
    let result = optimize_tuple_layout(&input);
    assert_eq!(result.size, 24);
    assert_eq!(result.align, 8);
    // int and float (both align 8) first, then bool and byte (align 1)
    assert_eq!(result.elements[0].original_index, 1); // int
    assert_eq!(result.elements[1].original_index, 3); // float
    assert_eq!(result.elements[2].original_index, 0); // bool
    assert_eq!(result.elements[3].original_index, 2); // byte
}

#[test]
fn test_three_element_tuple_memory_index_remapping() {
    // Verify codegen can remap .0/.1/.2 after reorder.
    let input = make_tuple(vec![
        field(0, bool_repr()),
        field(1, int_repr()),
        field(2, bool_repr()),
    ]);
    let result = optimize_tuple_layout(&input);
    // .0 (bool) → memory position 1
    assert_eq!(result.memory_index(0), Some(1));
    // .1 (int) → memory position 0
    assert_eq!(result.memory_index(1), Some(0));
    // .2 (bool) → memory position 2
    assert_eq!(result.memory_index(2), Some(2));
}

// -- min_tag_width tests (§07.1) --

use crate::enum_repr::min_tag_width;

#[test]
fn min_tag_width_zero_variants() {
    assert_eq!(min_tag_width(0), IntWidth::I8);
}

#[test]
fn min_tag_width_single_variant() {
    assert_eq!(min_tag_width(1), IntWidth::I8);
}

#[test]
fn min_tag_width_two_variants() {
    assert_eq!(min_tag_width(2), IntWidth::I8);
}

#[test]
fn min_tag_width_256_variants() {
    // 256 = 2^8, fits in u8 (values 0..255)
    assert_eq!(min_tag_width(256), IntWidth::I8);
}

#[test]
fn min_tag_width_257_variants_needs_i16() {
    // 257 > 256 → needs i16
    assert_eq!(min_tag_width(257), IntWidth::I16);
}

#[test]
fn min_tag_width_65536_variants() {
    // 65536 = 2^16, fits in u16
    assert_eq!(min_tag_width(65536), IntWidth::I16);
}

#[test]
fn min_tag_width_65537_variants_needs_i32() {
    assert_eq!(min_tag_width(65537), IntWidth::I32);
}

// ─── Niche analysis tests (§07.2) ──────────────────────────────

use crate::enum_repr::{EnumRepr, EnumTag, VariantRepr};
use crate::layout::niche::{
    find_enum_niches, find_niches, optimize_option_repr, optimize_result_repr,
};
use crate::struct_repr::{FatRepr, RcRepr};

fn ordering_repr() -> MachineRepr {
    MachineRepr::Ordering
}

fn char_repr() -> MachineRepr {
    MachineRepr::Char
}

fn str_repr() -> MachineRepr {
    MachineRepr::FatPointer(FatRepr::Str)
}

fn list_repr() -> MachineRepr {
    MachineRepr::FatPointer(FatRepr::Collection {
        element_repr: Box::new(int_repr()),
    })
}

fn rc_repr() -> MachineRepr {
    MachineRepr::RcPointer(RcRepr {
        rc_width: IntWidth::I64,
        atomic: false,
        inner: Box::new(MachineRepr::Unit),
        stack_promotable: false,
    })
}

#[test]
fn find_niches_bool_254() {
    let niches = find_niches(&bool_repr());
    assert_eq!(niches.len(), 1);
    assert_eq!(niches[0].available, 254);
    assert_eq!(niches[0].start, 2);
    assert_eq!(niches[0].field_index, 0);
}

#[test]
fn find_niches_ordering_253() {
    let niches = find_niches(&ordering_repr());
    assert_eq!(niches.len(), 1);
    assert_eq!(niches[0].available, 253);
    assert_eq!(niches[0].start, 3);
}

#[test]
fn find_niches_char_unicode() {
    let niches = find_niches(&char_repr());
    assert_eq!(niches.len(), 1);
    assert_eq!(niches[0].start, 0x11_0000);
    assert_eq!(
        niches[0].available,
        u128::from(0xFFFF_FFFFu32 - 0x10_FFFFu32)
    );
}

#[test]
fn find_niches_str_null_ptr() {
    let niches = find_niches(&str_repr());
    assert_eq!(niches.len(), 1);
    assert_eq!(niches[0].field_index, 2);
    assert_eq!(niches[0].available, 1);
    assert_eq!(niches[0].start, 0);
}

#[test]
fn find_niches_rc_pointer_null() {
    let niches = find_niches(&rc_repr());
    assert_eq!(niches.len(), 1);
    assert_eq!(niches[0].available, 1);
    assert_eq!(niches[0].start, 0);
}

#[test]
fn find_niches_byte_empty() {
    assert!(
        find_niches(&byte_repr()).is_empty(),
        "byte: all 256 values valid"
    );
}

#[test]
fn find_niches_int_i64_empty() {
    assert!(find_niches(&int_repr()).is_empty(), "i64: all values valid");
}

#[test]
fn find_niches_list_empty() {
    assert!(
        find_niches(&list_repr()).is_empty(),
        "[int]: null is valid for empty"
    );
}

#[test]
fn find_niches_float_empty() {
    assert!(
        find_niches(&float_repr()).is_empty(),
        "float niches skipped"
    );
}

#[test]
fn find_niches_unit_empty() {
    assert!(find_niches(&MachineRepr::Unit).is_empty());
}

#[test]
fn find_enum_niches_4_variant_i8_tag() {
    let e = EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I8,
        },
        variants: vec![
            VariantRepr {
                name: Name::from_raw(0),
                fields: vec![],
                size: 0,
                alignment: 1,
            },
            VariantRepr {
                name: Name::from_raw(1),
                fields: vec![],
                size: 0,
                alignment: 1,
            },
            VariantRepr {
                name: Name::from_raw(2),
                fields: vec![],
                size: 0,
                alignment: 1,
            },
            VariantRepr {
                name: Name::from_raw(3),
                fields: vec![],
                size: 0,
                alignment: 1,
            },
        ],
        size: 1,
        align: 1,
    };
    let niches = find_enum_niches(&e);
    assert_eq!(niches.len(), 1);
    assert_eq!(niches[0].start, 4);
    assert_eq!(niches[0].available, 252);
}

#[test]
fn option_bool_niche_1_byte() {
    let repr = optimize_option_repr(&bool_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(
            matches!(
                e.tag,
                EnumTag::Niche {
                    niche_value: 2,
                    niche_variant_idx: 0,
                    ..
                }
            ),
            "Option<bool> must use niche value 2 for None (variant 0)"
        );
        assert_eq!(e.size, 1, "Option<bool> must be 1 byte");
        assert_eq!(e.variants.len(), 2);
    } else {
        panic!("expected Enum, got {repr:?}");
    }
}

#[test]
fn option_ordering_niche_1_byte() {
    let repr = optimize_option_repr(&ordering_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(matches!(e.tag, EnumTag::Niche { niche_value: 3, .. }));
        assert_eq!(e.size, 1, "Option<Ordering> must be 1 byte");
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn option_char_niche_4_bytes() {
    let repr = optimize_option_repr(&char_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(matches!(
            e.tag,
            EnumTag::Niche {
                niche_value: 0x11_0000,
                ..
            }
        ));
        assert_eq!(e.size, 4, "Option<char> must be 4 bytes");
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn option_str_niche_24_bytes() {
    let repr = optimize_option_repr(&str_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(
            matches!(
                e.tag,
                EnumTag::Niche {
                    field_index: 2,
                    niche_value: 0,
                    ..
                }
            ),
            "Option<str> must use null data ptr niche"
        );
        assert_eq!(e.size, 24, "Option<str> must be 24 bytes (same as str)");
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn option_rc_pointer_niche_8_bytes() {
    let repr = optimize_option_repr(&rc_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(matches!(e.tag, EnumTag::Niche { niche_value: 0, .. }));
        assert_eq!(e.size, 8);
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn option_int_explicit_tag() {
    let repr = optimize_option_repr(&int_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(
            matches!(
                e.tag,
                EnumTag::Explicit {
                    width: IntWidth::I64
                }
            ),
            "Option<int> must use explicit i64 tag (no niche for i64)"
        );
        assert_eq!(
            e.size, 16,
            "Option<int> = {{i64 tag, i64 value}} = 16 bytes"
        );
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn option_list_explicit_tag() {
    let repr = optimize_option_repr(&list_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(
            matches!(e.tag, EnumTag::Explicit { .. }),
            "Option<[int]> must use explicit tag (empty list = null)"
        );
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn option_option_bool_niche_1_byte() {
    let inner = optimize_option_repr(&bool_repr());
    let outer = optimize_option_repr(&inner);
    if let MachineRepr::Enum(e) = &outer {
        assert!(
            matches!(
                e.tag,
                EnumTag::Niche {
                    niche_value: 3,
                    niche_variant_idx: 0,
                    ..
                }
            ),
            "Option<Option<bool>> must use niche value 3 for outer None"
        );
        assert_eq!(e.size, 1, "Option<Option<bool>> must be 1 byte");
    } else {
        panic!("expected Enum, got {outer:?}");
    }
}

#[test]
fn result_bool_ordering_niche() {
    let repr = optimize_result_repr(&bool_repr(), &ordering_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(
            matches!(
                e.tag,
                EnumTag::Niche {
                    niche_variant_idx: 1,
                    ..
                }
            ),
            "Result<bool, Ordering>: Err encoded via bool's niche"
        );
    } else {
        panic!("expected Enum");
    }
}

#[test]
fn result_int_int_explicit_tag() {
    let repr = optimize_result_repr(&int_repr(), &int_repr());
    if let MachineRepr::Enum(e) = &repr {
        assert!(
            matches!(e.tag, EnumTag::Explicit { .. }),
            "Result<int, int> must use explicit tag (neither has niches)"
        );
    } else {
        panic!("expected Enum");
    }
}
