//! Borrowed parameter iteration tests — F5: function parameter iteration.
//!
//! Dimension matrix: call count (1, 2, N) × iteration mode (full, break, yield)
//! × element type ([str], [int], struct) × caller context (own, COW, chained).

use crate::util::assert_aot_success;

// Single call × full iteration

#[test]
fn test_borrowed_str_list_single_call() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_str_list_single_call.ori"),
        "borrowed_str_list_single_call",
    );
}

#[test]
fn test_borrowed_int_list_single_call() {
    // [int] has scalar elements — no element-level RC, but the list buffer
    // itself is RC-managed. Verifies the fix doesn't break scalar iteration.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_int_list_single_call.ori"),
        "borrowed_int_list_single_call",
    );
}

#[test]
fn test_borrowed_struct_list_single_call() {
    // Struct with str field — element-level Drop involves field traversal.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_struct_list_single_call.ori"
        ),
        "borrowed_struct_list_single_call",
    );
}

// Two sequential calls (the original bug scenario)

#[test]
fn test_borrowed_str_list_two_calls() {
    // This was the original double-free: two calls to same function with
    // borrowed [str] param. Second call used freed memory.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_str_list_two_calls.ori"),
        "borrowed_str_list_two_calls",
    );
}

#[test]
fn test_borrowed_int_list_two_calls() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_int_list_two_calls.ori"),
        "borrowed_int_list_two_calls",
    );
}

// N calls in a loop

#[test]
fn test_borrowed_str_list_called_in_loop() {
    // Call the borrowing function multiple times from a for loop.
    // Stresses RC accounting across repeated borrows.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_str_list_called_in_loop.ori"
        ),
        "borrowed_str_list_called_in_loop",
    );
}

// Partial iteration (break) with borrowed param

#[test]
fn test_borrowed_str_list_partial_break_two_calls() {
    // Break mid-iteration, then call again. Verifies unconsumed elements
    // are not leaked and the list is still valid for the second call.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_str_list_partial_break_two_calls.ori"
        ),
        "borrowed_str_list_partial_break_two_calls",
    );
}

// Yield with borrowed param

#[test]
fn test_borrowed_str_list_yield_two_calls() {
    // Yield from borrowed param iteration, call twice.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_str_list_yield_two_calls.ori"
        ),
        "borrowed_str_list_yield_two_calls",
    );
}

// COW after borrowed call

#[test]
fn test_borrowed_param_then_cow_mutation() {
    // Pass list to borrowing function, then mutate with COW.
    // Verifies RC is correct for the COW copy-on-write path.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_param_then_cow_mutation.ori"
        ),
        "borrowed_param_then_cow_mutation",
    );
}

// Chained callees: A calls B which iterates

#[test]
fn test_chained_borrowed_callee() {
    // main → wrapper → iterate. The list passes through two borrowed params.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/chained_borrowed_callee.ori"),
        "chained_borrowed_callee",
    );
}

// Borrowed param: iterate + use list after loop

#[test]
fn test_borrowed_param_use_after_iteration() {
    // Use the borrowed list AFTER the for loop completes.
    // Verifies the list is still valid post-iteration.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_param_use_after_iteration.ori"
        ),
        "borrowed_param_use_after_iteration",
    );
}

// Borrowed param: two different lists passed to same function

#[test]
fn test_two_different_borrowed_lists() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/two_different_borrowed_lists.ori"),
        "two_different_borrowed_lists",
    );
}

// Borrowed param: map iteration with string keys

#[test]
fn test_borrowed_map_str_keys_two_calls() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_map_str_keys_two_calls.ori"),
        "borrowed_map_str_keys_two_calls",
    );
}

// Combined scenarios: borrowed param + other features

#[test]
fn test_borrowed_param_break_then_full_iteration() {
    // First call breaks early, second iterates fully. Verifies the list
    // is intact after a partial iteration via borrowed param.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_param_break_then_full_iteration.ori"
        ),
        "borrowed_param_break_then_full_iteration",
    );
}

#[test]
fn test_borrowed_param_yield_then_iterate_result() {
    // yield from borrowed param produces a new list, then iterate that too.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_param_yield_then_iterate_result.ori"
        ),
        "borrowed_param_yield_then_iterate_result",
    );
}

#[test]
fn test_borrowed_struct_list_two_calls_with_field_access() {
    // Struct with str field, called twice. Exercises element-level Drop
    // through field traversal on a borrowed collection.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_struct_list_two_calls_with_field_access.ori"),
        "borrowed_struct_list_two_calls_with_field_access",
    );
}

#[test]
fn test_borrowed_param_mixed_callers() {
    // Same function called from two different callers with different lists.
    // Verifies no cross-contamination of borrowed references.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_param_mixed_callers.ori"),
        "borrowed_param_mixed_callers",
    );
}

#[test]
fn test_borrowed_param_iterate_then_index() {
    // Iterate borrowed list, then index into it. Both accesses in same callee.
    assert_aot_success(
        include_str!(
            "../fixtures/fat_ptr_iter/function_param/borrowed_param_iterate_then_index.ori"
        ),
        "borrowed_param_iterate_then_index",
    );
}

#[test]
fn test_borrowed_empty_list_iteration() {
    // Edge case: empty list passed to borrowing iterator function.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/function_param/borrowed_empty_list_iteration.ori"),
        "borrowed_empty_list_iteration",
    );
}
