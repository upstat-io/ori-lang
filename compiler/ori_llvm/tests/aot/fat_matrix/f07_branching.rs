//! F07: Branching — fat pointer types used in if/else expressions.
//!
//! Tests select vs branch emission, phi node merging for fat pointer results,
//! and correct RC on both branches.

use crate::util::assert_aot_success;

// T5: String from if/else branches
#[test]
fn test_fm_branch_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_str.ori"),
        "fm_branch_str",
    );
}

// T5: Heap string from conditional
#[test]
fn test_fm_branch_str_heap() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_str_heap.ori"),
        "fm_branch_str_heap",
    );
}

// T6: List from conditional
#[test]
fn test_fm_branch_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_list_scalar.ori"),
        "fm_branch_list_scalar",
    );
}

// T7: List of strings from conditional
#[test]
fn test_fm_branch_list_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_list_fat.ori"),
        "fm_branch_list_fat",
    );
}

// T8: Struct from conditional
#[test]
fn test_fm_branch_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_struct_scalar.ori"),
        "fm_branch_struct_scalar",
    );
}

// T9: Struct with fat fields from conditional
#[test]
fn test_fm_branch_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_struct_fat.ori"),
        "fm_branch_struct_fat",
    );
}

// T15: Option<int> from conditional
#[test]
fn test_fm_branch_option_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_option_int.ori"),
        "fm_branch_option_int",
    );
}

// T16: Option<str> from conditional
#[test]
fn test_fm_branch_option_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_option_str.ori"),
        "fm_branch_option_str",
    );
}

// T17: Map from conditional
#[test]
fn test_fm_branch_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_map.ori"),
        "fm_branch_map",
    );
}

// T18: Tuple from conditional
#[test]
fn test_fm_branch_tuple() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_tuple.ori"),
        "fm_branch_tuple",
    );
}

// Nested branching with fat values
#[test]
fn test_fm_branch_nested() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f07_branching/fm_branch_nested.ori"),
        "fm_branch_nested",
    );
}
