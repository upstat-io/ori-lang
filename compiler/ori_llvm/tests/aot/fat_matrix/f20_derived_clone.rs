//! F20: Derived Clone — cloning types containing fat pointer fields.
//!
//! Tests Clone codegen for structs and sum types whose fields include strings,
//! lists, and other fat pointer types. Clone must correctly RC-increment all
//! heap-allocated fields.

use crate::util::assert_aot_success;

// T4/T5: Clone struct with str field
#[test]
fn test_fm_clone_struct_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_struct_str.ori"),
        "fm_clone_struct_str",
    );
}

// T5: Clone struct with heap str field
#[test]
fn test_fm_clone_struct_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_struct_str_heap.ori"),
        "fm_clone_struct_str_heap",
    );
}

// T6: Clone struct with list of scalars
#[test]
fn test_fm_clone_struct_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_struct_list_scalar.ori"),
        "fm_clone_struct_list_scalar",
    );
}

// T7: Clone struct with list of fat pointers
#[test]
fn test_fm_clone_struct_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_struct_list_fat.ori"),
        "fm_clone_struct_list_fat",
    );
}

// T9: Clone struct with nested fat fields
#[test]
fn test_fm_clone_nested_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_nested_fat.ori"),
        "fm_clone_nested_fat",
    );
}

// Multiple fat fields — both must be cloned
#[test]
fn test_fm_clone_multi_fat_fields() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_multi_fat_fields.ori"),
        "fm_clone_multi_fat_fields",
    );
}

// Clone used after original is consumed (RC independence)
#[test]
fn test_fm_clone_independence() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_independence.ori"),
        "fm_clone_independence",
    );
}

// Clone with map field
#[test]
fn test_fm_clone_map_field() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f20_derived_clone/fm_clone_map_field.ori"),
        "fm_clone_map_field",
    );
}
