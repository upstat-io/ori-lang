//! F18: Multiple Values — multiple fat pointer values of the same type in scope.
//!
//! Tests RC tracking correctness when multiple values compete for cleanup,
//! drop ordering, and no interference between independent values.

use crate::util::assert_aot_success;

// Multiple heap strings in scope
#[test]
fn test_fm_multi_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f18_multiple_values/fm_multi_str.ori"),
        "fm_multi_str",
    );
}

// Multiple lists in scope
#[test]
fn test_fm_multi_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f18_multiple_values/fm_multi_list.ori"),
        "fm_multi_list",
    );
}

// Multiple structs with fat fields
#[test]
fn test_fm_multi_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f18_multiple_values/fm_multi_struct_fat.ori"),
        "fm_multi_struct_fat",
    );
}

// Multiple maps
#[test]
fn test_fm_multi_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f18_multiple_values/fm_multi_map.ori"),
        "fm_multi_map",
    );
}

// Mixed fat types in same scope
#[test]
fn test_fm_multi_mixed() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f18_multiple_values/fm_multi_mixed.ori"),
        "fm_multi_mixed",
    );
}
