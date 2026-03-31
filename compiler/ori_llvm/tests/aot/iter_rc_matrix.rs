//! Iterator-collection RC ownership matrix tests.
//!
//! Comprehensive combinatorial coverage: 6 element types × 8 iteration
//! patterns × 2 loop variants = 88 tests. Exercises every RC-relevant
//! combination to prevent regression.
//!
//! Matrix dimensions:
//!   Loop:    for-do (P1-P2, P4-P8), for-yield (P1-P8)
//!   Type:    E1=str, E2=[int] nested, E3=Option<str>, E4=closure, E5=struct, E6=map
//!   Pattern: P1=full, P2=break, P3=yield-transform, P4=two-call, P5=nested,
//!            P6=guard, P7=unwind, P8=continue
//!
//! E7 (Set<str>) is excluded — Set<str> not yet implemented in AOT.
//! E2×P5 for-yield excluded — nested yield of [int] collapses to flat int; covered by P1.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// -----------------------------------------------------------------------
// E1: [str] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_str_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_str_full.ori"),
        "iter_rc_for_do_str_full",
    );
}

#[test]
fn test_iter_rc_for_do_str_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_str_break.ori"),
        "iter_rc_for_do_str_break",
    );
}

#[test]
fn test_iter_rc_for_do_str_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_str_two_call.ori"),
        "iter_rc_for_do_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_str_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_str_nested.ori"),
        "iter_rc_for_do_str_nested",
    );
}

#[test]
fn test_iter_rc_for_do_str_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_str_guard.ori"),
        "iter_rc_for_do_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_str_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_str_unwind.ori"),
        "iter_rc_for_do_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_str_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_str_continue.ori"),
        "iter_rc_for_do_str_continue",
    );
}

// -----------------------------------------------------------------------
// E2: [[int]] nested list — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_nested_list_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_nested_list_full.ori"),
        "iter_rc_for_do_nested_list_full",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_nested_list_break.ori"),
        "iter_rc_for_do_nested_list_break",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_nested_list_two_call.ori"),
        "iter_rc_for_do_nested_list_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_nested_list_nested.ori"),
        "iter_rc_for_do_nested_list_nested",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_nested_list_guard.ori"),
        "iter_rc_for_do_nested_list_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_nested_list_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_nested_list_unwind.ori"),
        "iter_rc_for_do_nested_list_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_nested_list_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_nested_list_continue.ori"),
        "iter_rc_for_do_nested_list_continue",
    );
}

// -----------------------------------------------------------------------
// E3: [Option<str>] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_option_str_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_option_str_full.ori"),
        "iter_rc_for_do_option_str_full",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_option_str_break.ori"),
        "iter_rc_for_do_option_str_break",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_option_str_two_call.ori"),
        "iter_rc_for_do_option_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_option_str_nested.ori"),
        "iter_rc_for_do_option_str_nested",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_option_str_guard.ori"),
        "iter_rc_for_do_option_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_option_str_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_option_str_unwind.ori"),
        "iter_rc_for_do_option_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_option_str_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_option_str_continue.ori"),
        "iter_rc_for_do_option_str_continue",
    );
}

// -----------------------------------------------------------------------
// E4: [closure] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_closure_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_closure_full.ori"),
        "iter_rc_for_do_closure_full",
    );
}

#[test]
fn test_iter_rc_for_do_closure_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_closure_break.ori"),
        "iter_rc_for_do_closure_break",
    );
}

#[test]
fn test_iter_rc_for_do_closure_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_closure_two_call.ori"),
        "iter_rc_for_do_closure_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_closure_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_closure_nested.ori"),
        "iter_rc_for_do_closure_nested",
    );
}

#[test]
fn test_iter_rc_for_do_closure_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_closure_guard.ori"),
        "iter_rc_for_do_closure_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_closure_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_closure_unwind.ori"),
        "iter_rc_for_do_closure_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_closure_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_closure_continue.ori"),
        "iter_rc_for_do_closure_continue",
    );
}

// -----------------------------------------------------------------------
// E5: [struct with str field] — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_struct_str_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_struct_str_full.ori"),
        "iter_rc_for_do_struct_str_full",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_struct_str_break.ori"),
        "iter_rc_for_do_struct_str_break",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_struct_str_two_call.ori"),
        "iter_rc_for_do_struct_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_struct_str_nested.ori"),
        "iter_rc_for_do_struct_str_nested",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_struct_str_guard.ori"),
        "iter_rc_for_do_struct_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_struct_str_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_struct_str_unwind.ori"),
        "iter_rc_for_do_struct_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_struct_str_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_struct_str_continue.ori"),
        "iter_rc_for_do_struct_str_continue",
    );
}

// -----------------------------------------------------------------------
// E6: {str: int} map — for-do patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_do_map_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_map_full.ori"),
        "iter_rc_for_do_map_full",
    );
}

#[test]
fn test_iter_rc_for_do_map_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_map_break.ori"),
        "iter_rc_for_do_map_break",
    );
}

#[test]
fn test_iter_rc_for_do_map_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_map_two_call.ori"),
        "iter_rc_for_do_map_two_call",
    );
}

#[test]
fn test_iter_rc_for_do_map_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_map_guard.ori"),
        "iter_rc_for_do_map_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_do_map_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_map_unwind.ori"),
        "iter_rc_for_do_map_unwind",
    );
}

#[test]
fn test_iter_rc_for_do_map_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_do_map_continue.ori"),
        "iter_rc_for_do_map_continue",
    );
}

// -----------------------------------------------------------------------
// E1: [str] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_str_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_full.ori"),
        "iter_rc_for_yield_str_full",
    );
}

#[test]
fn test_iter_rc_for_yield_str_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_break.ori"),
        "iter_rc_for_yield_str_break",
    );
}

#[test]
fn test_iter_rc_for_yield_str_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_yield_transform.ori"),
        "iter_rc_for_yield_str_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_str_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_two_call.ori"),
        "iter_rc_for_yield_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_str_nested() {
    // Nested for-yield: outer yields inner lists, which are then iterated.
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_nested.ori"),
        "iter_rc_for_yield_str_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_str_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_guard.ori"),
        "iter_rc_for_yield_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_str_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_unwind.ori"),
        "iter_rc_for_yield_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_str_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_str_continue.ori"),
        "iter_rc_for_yield_str_continue",
    );
}

// -----------------------------------------------------------------------
// E2: [[int]] nested list — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_nested_list_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_nested_list_full.ori"),
        "iter_rc_for_yield_nested_list_full",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_nested_list_break.ori"),
        "iter_rc_for_yield_nested_list_break",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_nested_list_yield_transform.ori"),
        "iter_rc_for_yield_nested_list_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_nested_list_two_call.ori"),
        "iter_rc_for_yield_nested_list_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_nested_list_guard.ori"),
        "iter_rc_for_yield_nested_list_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_nested_list_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_nested_list_unwind.ori"),
        "iter_rc_for_yield_nested_list_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_nested_list_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_nested_list_continue.ori"),
        "iter_rc_for_yield_nested_list_continue",
    );
}

// -----------------------------------------------------------------------
// E3: [Option<str>] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_option_str_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_full.ori"),
        "iter_rc_for_yield_option_str_full",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_break.ori"),
        "iter_rc_for_yield_option_str_break",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_yield_transform.ori"),
        "iter_rc_for_yield_option_str_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_two_call.ori"),
        "iter_rc_for_yield_option_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_nested.ori"),
        "iter_rc_for_yield_option_str_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_guard.ori"),
        "iter_rc_for_yield_option_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_option_str_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_unwind.ori"),
        "iter_rc_for_yield_option_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_option_str_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_option_str_continue.ori"),
        "iter_rc_for_yield_option_str_continue",
    );
}

// -----------------------------------------------------------------------
// E4: [closure] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_closure_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_full.ori"),
        "iter_rc_for_yield_closure_full",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_break.ori"),
        "iter_rc_for_yield_closure_break",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_yield_transform.ori"),
        "iter_rc_for_yield_closure_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_two_call.ori"),
        "iter_rc_for_yield_closure_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_nested.ori"),
        "iter_rc_for_yield_closure_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_guard.ori"),
        "iter_rc_for_yield_closure_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_closure_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_unwind.ori"),
        "iter_rc_for_yield_closure_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_closure_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_closure_continue.ori"),
        "iter_rc_for_yield_closure_continue",
    );
}

// -----------------------------------------------------------------------
// E5: [struct with str field] — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_struct_str_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_full.ori"),
        "iter_rc_for_yield_struct_str_full",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_break.ori"),
        "iter_rc_for_yield_struct_str_break",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_yield_transform.ori"),
        "iter_rc_for_yield_struct_str_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_two_call.ori"),
        "iter_rc_for_yield_struct_str_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_nested() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_nested.ori"),
        "iter_rc_for_yield_struct_str_nested",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_guard.ori"),
        "iter_rc_for_yield_struct_str_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_struct_str_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_unwind.ori"),
        "iter_rc_for_yield_struct_str_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_struct_str_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_struct_str_continue.ori"),
        "iter_rc_for_yield_struct_str_continue",
    );
}

// -----------------------------------------------------------------------
// E6: {str: int} map — for-yield patterns
// -----------------------------------------------------------------------

#[test]
fn test_iter_rc_for_yield_map_full() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_map_full.ori"),
        "iter_rc_for_yield_map_full",
    );
}

#[test]
fn test_iter_rc_for_yield_map_break() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_map_break.ori"),
        "iter_rc_for_yield_map_break",
    );
}

#[test]
fn test_iter_rc_for_yield_map_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_map_yield_transform.ori"),
        "iter_rc_for_yield_map_yield_transform",
    );
}

#[test]
fn test_iter_rc_for_yield_map_two_call() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_map_two_call.ori"),
        "iter_rc_for_yield_map_two_call",
    );
}

#[test]
fn test_iter_rc_for_yield_map_guard() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_map_guard.ori"),
        "iter_rc_for_yield_map_guard",
    );
}

#[test]
#[ignore = "catch() type inference bug: returns Result<() -> T, str> instead of Result<T, str>"]
fn test_iter_rc_for_yield_map_unwind() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_map_unwind.ori"),
        "iter_rc_for_yield_map_unwind",
    );
}

#[test]
fn test_iter_rc_for_yield_map_continue() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_for_yield_map_continue.ori"),
        "iter_rc_for_yield_map_continue",
    );
}

// Edge case tests

#[test]
fn test_iter_rc_edge_empty_str_for_do() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_edge_empty_str_for_do.ori"),
        "iter_rc_edge_empty_str_for_do",
    );
}

#[test]
fn test_iter_rc_edge_empty_str_for_yield() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_edge_empty_str_for_yield.ori"),
        "iter_rc_edge_empty_str_for_yield",
    );
}

#[test]
fn test_iter_rc_edge_single_str_for_do() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_edge_single_str_for_do.ori"),
        "iter_rc_edge_single_str_for_do",
    );
}

#[test]
fn test_iter_rc_edge_single_str_for_yield() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_edge_single_str_for_yield.ori"),
        "iter_rc_edge_single_str_for_yield",
    );
}

#[test]
fn test_iter_rc_edge_large_str_for_yield() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_edge_large_str_for_yield.ori"),
        "iter_rc_edge_large_str_for_yield",
    );
}

#[test]
fn test_iter_rc_edge_empty_map_for_do() {
    assert_aot_success(
        include_str!("fixtures/iter_rc_matrix/iter_rc_edge_empty_map_for_do.ori"),
        "iter_rc_edge_empty_map_for_do",
    );
}
