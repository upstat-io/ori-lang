//! F08: For Loop Iteration — iterating over collections containing fat pointer elements.
//!
//! This is the J15 bug area: iterating over `[str]` and other fat-pointer
//! collections required proper `elem_dec_fn` and iterator ownership contracts.
//! Tests both for-do and for-yield with fat pointer element types.

use crate::util::assert_aot_success;

// T6: Iterate over [int] (for-do)
#[test]
fn test_fm_for_list_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_list_scalar.ori"),
        "fm_for_list_scalar",
    );
}

// T7: Iterate over [str] (for-do)
#[test]
fn test_fm_for_list_str_do() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_list_str_do.ori"),
        "fm_for_list_str_do",
    );
}

// T7: Iterate over [str] (for-yield)
#[test]
fn test_fm_for_list_str_yield() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_list_str_yield.ori"),
        "fm_for_list_str_yield",
    );
}

// T7: Iterate over [str] with break
#[test]
fn test_fm_for_list_str_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_list_str_break.ori"),
        "fm_for_list_str_break",
    );
}

// T7: Iterate over [str] twice (RC correctness)
#[test]
fn test_fm_for_list_str_two_iterations() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_list_str_two_iterations.ori"),
        "fm_for_list_str_two_iterations",
    );
}

// T8: Iterate over [Point] (struct with scalar fields)
#[test]
fn test_fm_for_list_struct_scalar() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_list_struct_scalar.ori"),
        "fm_for_list_struct_scalar",
    );
}

// T9: Iterate over [Named] (struct with fat fields)
#[test]
fn test_fm_for_list_struct_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_list_struct_fat.ori"),
        "fm_for_list_struct_fat",
    );
}

// T17: Iterate over map (for-do)
#[test]
fn test_fm_for_map_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_map_str.ori"),
        "fm_for_map_str",
    );
}

// Nested for loops with fat elements
#[test]
fn test_fm_for_nested_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_nested_fat.ori"),
        "fm_for_nested_fat",
    );
}

// For-yield with fat element transformation
#[test]
fn test_fm_for_yield_fat_transform() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f08_for_loop/fm_for_yield_fat_transform.ori"),
        "fm_for_yield_fat_transform",
    );
}
