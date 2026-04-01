//! Nested list iteration tests — T2 (`[[int]]`) and T3 (`[[str]]`).

use crate::util::assert_aot_success;

// T2: [[int]] — inner lists are RC-managed buffers

#[test]
fn test_nested_list_iteration() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/nested_list/nested_list_iteration.ori"),
        "nested_list_iteration",
    );
}

// T3: [[str]] — doubly-nested fat pointers (inner str elements need elem_dec_fn)

#[test]
fn test_nested_str_list_iteration() {
    // Both the outer list and inner lists need elem_dec_fn cleanup.
    // Outer: elem_dec_fn decs inner [str] buffers.
    // Inner: elem_dec_fn decs str elements within each inner list.
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/nested_list/nested_str_list_iteration.ori"),
        "nested_str_list_iteration",
    );
}

// T3-F2: [[str]] with break in outer loop — un-consumed outer elements cleaned up.
// Break after first outer iteration: second and third inner [str] lists are un-consumed
// and must have both their buffer RC and element (str) RC decremented.

#[test]
fn test_nested_str_list_break() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/nested_list/nested_str_list_break.ori"),
        "nested_str_list_break",
    );
}

// T3-F5: [[str]] passed to function twice — shared ownership, no double-free.

#[test]
fn test_nested_str_list_two_calls() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/nested_list/nested_str_list_two_calls.ori"),
        "nested_str_list_two_calls",
    );
}
