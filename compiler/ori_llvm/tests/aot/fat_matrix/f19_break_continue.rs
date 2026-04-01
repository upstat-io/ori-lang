//! F19: Break/Continue — early exit from loops with fat pointer values in scope.
//!
//! Tests cleanup on break (remaining iterator elements and in-scope fat values),
//! continue semantics with fat values, and correct RC in both paths.

use crate::util::assert_aot_success;

// Break from loop with [str] — remaining elements must be cleaned up
#[test]
fn test_fm_break_str_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f19_break_continue/fm_break_str_list.ori"),
        "fm_break_str_list",
    );
}

// Continue in loop with [str]
#[test]
fn test_fm_continue_str_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f19_break_continue/fm_continue_str_list.ori"),
        "fm_continue_str_list",
    );
}

// Break with fat value created inside loop
#[test]
fn test_fm_break_inner_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f19_break_continue/fm_break_inner_fat.ori"),
        "fm_break_inner_fat",
    );
}

// Continue with fat value created inside loop
#[test]
fn test_fm_continue_inner_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f19_break_continue/fm_continue_inner_fat.ori"),
        "fm_continue_inner_fat",
    );
}

// Break from for-yield with [str]
#[test]
fn test_fm_break_yield_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f19_break_continue/fm_break_yield_str.ori"),
        "fm_break_yield_str",
    );
}

// Nested loops with break — outer fat values preserved
#[test]
fn test_fm_break_nested_fat() {
    assert_aot_success(
        include_str!("../fixtures/fat_matrix/f19_break_continue/fm_break_nested_fat.ori"),
        "fm_break_nested_fat",
    );
}
