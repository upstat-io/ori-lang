//! Control flow regression matrix — element type × iteration pattern cross-product.
//!
//! Covers: str, [int], closures, structs, Option<str> × full, break, yield,
//! two-call, nested, unwind patterns.

use crate::util::assert_aot_success;

#[test]
fn test_matrix_nested_list_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_nested_list_break.ori"),
        "matrix_nested_list_break",
    );
}

/// [[int]] passed to two functions — verifies RC balance across multiple
/// iteration passes over nested collections.
#[test]
fn test_matrix_nested_list_two_calls() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_nested_list_two_calls.ori"),
        "matrix_nested_list_two_calls",
    );
}

/// [[int]] with yield — outer for-yield produces lengths of inner lists.
#[test]
fn test_matrix_nested_list_yield() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_nested_list_yield.ori"),
        "matrix_nested_list_yield",
    );
}

/// Struct with Drop field + break — partially consumed iteration.
#[test]
fn test_matrix_struct_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_struct_break.ori"),
        "matrix_struct_break",
    );
}

/// Struct with Drop field + yield — extracts labels from structs.
#[test]
fn test_matrix_struct_yield() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_struct_yield.ori"),
        "matrix_struct_yield",
    );
}

/// Closure list + break — partially consumed closure iteration.
#[test]
fn test_matrix_closure_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_closure_break.ori"),
        "matrix_closure_break",
    );
}

/// Option<str> + yield — extracts string lengths from Some values.
#[test]
fn test_matrix_option_str_yield() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_option_str_yield.ori"),
        "matrix_option_str_yield",
    );
}

/// [str] + unwind during nested function + catch — most complex
/// scenario combining fat pointers, iteration, and error recovery.
#[test]
fn test_matrix_str_nested_unwind() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/matrix_str_nested_unwind.ori"),
        "matrix_str_nested_unwind",
    );
}

// F4: For-guard (for x in coll if predicate do body)

/// T1-F4: [str] for-do with guard — elements failing the guard must be cleaned up.
#[test]
fn test_str_list_for_do_guard() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/str_list_for_do_guard.ori"),
        "str_list_for_do_guard",
    );
}

/// T2-F4: [[int]] for-do with guard — inner list elements skipped by guard cleaned up.
#[test]
fn test_nested_list_for_do_guard() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/nested_list_for_do_guard.ori"),
        "nested_list_for_do_guard",
    );
}

// F7: Continue (skip element)

/// T1-F7: [str] for-do with continue — skipped str elements must be cleaned up.
#[test]
fn test_str_list_for_do_continue() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/str_list_for_do_continue.ori"),
        "str_list_for_do_continue",
    );
}

/// T2-F7: [[int]] for-do with continue — skipped inner lists must be cleaned up.
#[test]
fn test_nested_list_for_do_continue() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/nested_list_for_do_continue.ori"),
        "nested_list_for_do_continue",
    );
}

// F8: Iteration in match arm

/// T1-F8: [str] iteration inside a match arm — cleanup regardless of which arm executes.
#[test]
fn test_str_list_iteration_in_match() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/str_list_iteration_in_match.ori"),
        "str_list_iteration_in_match",
    );
}

// F9: Slice iteration

/// T1-F9: [str] slice iteration — create list, take slice, iterate slice.
/// Verifies `elem_dec_fn` is read from the ORIGINAL buffer's header.
#[test]
fn test_str_list_slice_iteration() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/control_flow/str_list_slice_iteration.ori"),
        "str_list_slice_iteration",
    );
}
