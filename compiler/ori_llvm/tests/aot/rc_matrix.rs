//! RC Matrix Tests — Leak Regression Guard
//!
//! Systematically tests RC correctness across the cross-product of:
//! - 5 value types: str, [int], [str], {str: int}, struct w/ heap fields
//! - 3 loop patterns: for-range, manual loop (≈while), for+break
//! - 5 scope contexts: simple scope, if-else, match, function arg, function return
//! - 10 nested/composed patterns
//!
//! Every test uses `assert_aot_success` which enables `ORI_CHECK_LEAKS=1`.
//! Exit code 0 = correct result + zero leaks. Exit code 2 = leak detected.

use crate::util::assert_aot_success;

// ── 04.2 Value Type × Loop Pattern Matrix (15 tests) ──
//
// Each test reassigns a heap-allocated variable 30 times inside a loop.
// The old value must be RC-decremented on each reassignment.

// str × for-range
#[test]
fn test_matrix_str_for_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_for_loop.ori"),
        "matrix_str_for_loop",
    );
}

// str × manual loop (while equivalent)
#[test]
fn test_matrix_str_while_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_while_loop.ori"),
        "matrix_str_while_loop",
    );
}

// str × for+break (early exit)
#[test]
fn test_matrix_str_loop_break() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_loop_break.ori"),
        "matrix_str_loop_break",
    );
}

// [int] × for-range
#[test]
fn test_matrix_list_int_for_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_int_for_loop.ori"),
        "matrix_list_int_for_loop",
    );
}

// [int] × manual loop
#[test]
fn test_matrix_list_int_while_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_int_while_loop.ori"),
        "matrix_list_int_while_loop",
    );
}

// [int] × for+break
#[test]
fn test_matrix_list_int_loop_break() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_int_loop_break.ori"),
        "matrix_list_int_loop_break",
    );
}

// [str] × for-range
#[test]
fn test_matrix_list_str_for_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_for_loop.ori"),
        "matrix_list_str_for_loop",
    );
}

// [str] × manual loop
#[test]
fn test_matrix_list_str_while_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_while_loop.ori"),
        "matrix_list_str_while_loop",
    );
}

// [str] × for+break
#[test]
fn test_matrix_list_str_loop_break() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_loop_break.ori"),
        "matrix_list_str_loop_break",
    );
}

// {str: int} × for-range
#[test]
fn test_matrix_map_for_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_for_loop.ori"),
        "matrix_map_for_loop",
    );
}

// {str: int} × manual loop
#[test]
fn test_matrix_map_while_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_while_loop.ori"),
        "matrix_map_while_loop",
    );
}

// {str: int} × for+break
#[test]
fn test_matrix_map_loop_break() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_loop_break.ori"),
        "matrix_map_loop_break",
    );
}

// struct w/ heap fields × for-range
#[test]
fn test_matrix_struct_for_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_for_loop.ori"),
        "matrix_struct_for_loop",
    );
}

// struct w/ heap fields × manual loop
#[test]
fn test_matrix_struct_while_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_while_loop.ori"),
        "matrix_struct_while_loop",
    );
}

// struct w/ heap fields × for+break
#[test]
fn test_matrix_struct_loop_break() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_loop_break.ori"),
        "matrix_struct_loop_break",
    );
}

// ── 04.3 Value Type × Scope Pattern Matrix (25 tests) ──
//
// Each test creates a heap value in a specific scope context and verifies
// correct cleanup when the scope ends.

// str × simple scope
#[test]
fn test_matrix_str_scope() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_scope.ori"),
        "matrix_str_scope",
    );
}

// str × if-else
#[test]
fn test_matrix_str_if_else() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_if_else.ori"),
        "matrix_str_if_else",
    );
}

// str × match
#[test]
fn test_matrix_str_match() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_match.ori"),
        "matrix_str_match",
    );
}

// str × function arg
#[test]
fn test_matrix_str_arg() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_arg.ori"),
        "matrix_str_arg",
    );
}

// str × function return
#[test]
fn test_matrix_str_return() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_str_return.ori"),
        "matrix_str_return",
    );
}

// [int] × simple scope
#[test]
fn test_matrix_list_scope() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_scope.ori"),
        "matrix_list_scope",
    );
}

// [int] × if-else
#[test]
fn test_matrix_list_if_else() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_if_else.ori"),
        "matrix_list_if_else",
    );
}

// [int] × match
#[test]
fn test_matrix_list_match() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_match.ori"),
        "matrix_list_match",
    );
}

// [int] × function arg
#[test]
fn test_matrix_list_arg() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_arg.ori"),
        "matrix_list_arg",
    );
}

// [int] × function return
#[test]
fn test_matrix_list_return() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_return.ori"),
        "matrix_list_return",
    );
}

// [str] × simple scope
#[test]
fn test_matrix_list_str_scope() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_scope.ori"),
        "matrix_list_str_scope",
    );
}

// [str] × if-else
#[test]
fn test_matrix_list_str_if_else() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_if_else.ori"),
        "matrix_list_str_if_else",
    );
}

// [str] × match
#[test]
fn test_matrix_list_str_match() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_match.ori"),
        "matrix_list_str_match",
    );
}

// [str] × function arg
#[test]
fn test_matrix_list_str_arg() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_arg.ori"),
        "matrix_list_str_arg",
    );
}

// [str] × function return
#[test]
fn test_matrix_list_str_return() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_str_return.ori"),
        "matrix_list_str_return",
    );
}

// {str: int} × simple scope
#[test]
fn test_matrix_map_scope() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_scope.ori"),
        "matrix_map_scope",
    );
}

// {str: int} × if-else
#[test]
fn test_matrix_map_if_else() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_if_else.ori"),
        "matrix_map_if_else",
    );
}

// {str: int} × match
#[test]
fn test_matrix_map_match() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_match.ori"),
        "matrix_map_match",
    );
}

// {str: int} × function arg
#[test]
fn test_matrix_map_arg() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_arg.ori"),
        "matrix_map_arg",
    );
}

// {str: int} × function return
#[test]
fn test_matrix_map_return() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_map_return.ori"),
        "matrix_map_return",
    );
}

// struct w/ heap × simple scope
#[test]
fn test_matrix_struct_scope() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_scope.ori"),
        "matrix_struct_scope",
    );
}

// struct w/ heap × if-else
#[test]
fn test_matrix_struct_if_else() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_if_else.ori"),
        "matrix_struct_if_else",
    );
}

// struct w/ heap × match
#[test]
fn test_matrix_struct_match() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_match.ori"),
        "matrix_struct_match",
    );
}

// struct w/ heap × function arg
#[test]
fn test_matrix_struct_arg() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_arg.ori"),
        "matrix_struct_arg",
    );
}

// struct w/ heap × function return
#[test]
fn test_matrix_struct_return() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_return.ori"),
        "matrix_struct_return",
    );
}

// ── 04.4 Nested & Composed Pattern Matrix (10 tests) ──
//
// Tests combinations that compose multiple dimensions — highest risk.

// Struct containing [int] reassigned in loop
#[test]
fn test_matrix_struct_with_list_in_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_struct_with_list_in_loop.ori"),
        "matrix_struct_with_list_in_loop",
    );
}

// [str] with push in loop (nested RC: list + string elements)
#[test]
fn test_matrix_list_of_strings_in_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_list_of_strings_in_loop.ori"),
        "matrix_list_of_strings_in_loop",
    );
}

// String conditionally updated in loop
#[test]
fn test_matrix_string_in_if_else_in_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_string_in_if_else_in_loop.ori"),
        "matrix_string_in_if_else_in_loop",
    );
}

// Create slice, use, let both slice and original drop
#[test]
fn test_matrix_slice_in_scope() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_slice_in_scope.ori"),
        "matrix_slice_in_scope",
    );
}

// Create slices in a loop
#[test]
fn test_matrix_slice_in_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_slice_in_loop.ori"),
        "matrix_slice_in_loop",
    );
}

// Multiple independent heap variables in one scope
#[test]
fn test_matrix_multiple_heap_locals() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_multiple_heap_locals.ori"),
        "matrix_multiple_heap_locals",
    );
}

// Shadow a heap variable with a new heap value
#[test]
fn test_matrix_heap_var_shadowing() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_heap_var_shadowing.ori"),
        "matrix_heap_var_shadowing",
    );
}

// Lambda capturing a heap string, called, then dropped
#[test]
fn test_matrix_closure_captures_string() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_closure_captures_string.ori"),
        "matrix_closure_captures_string",
    );
}

// Lambda capturing a [int], called, then dropped
#[test]
fn test_matrix_closure_captures_list() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_closure_captures_list.ori"),
        "matrix_closure_captures_list",
    );
}

// Lambda created inside loop body capturing loop variable
#[test]
fn test_matrix_closure_in_loop() {
    assert_aot_success(
        include_str!("fixtures/rc_matrix/matrix_closure_in_loop.ori"),
        "matrix_closure_in_loop",
    );
}
