//! Unwind path tests — panic during iteration with catch recovery.

use crate::util::assert_aot_success;

#[test]
fn test_unwind_panic_during_str_iteration() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_panic_during_str_iteration.ori"),
        "unwind_panic_during_str_iteration",
    );
}

/// Panic during iteration, then reuse the list — verifies RC is correct
/// after unwind (list still accessible, no double-free).
#[test]
fn test_unwind_list_reusable_after_catch() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_list_reusable_after_catch.ori"),
        "unwind_list_reusable_after_catch",
    );
}

/// Multiple invoke calls in one function — panic at second call, verify
/// cleanup is correct for both call sites.
#[test]
fn test_unwind_multiple_invokes_with_panic() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_multiple_invokes_with_panic.ori"),
        "unwind_multiple_invokes_with_panic",
    );
}

/// Panic inside nested function call chain — A calls B calls C, C panics.
/// Verifies unwind cleanup propagates correctly through multiple frames.
#[test]
fn test_unwind_nested_call_chain_panic() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_nested_call_chain_panic.ori"),
        "unwind_nested_call_chain_panic",
    );
}

/// Partial iteration + break, then panic in separate call — verifies
/// that break cleanup and unwind cleanup are independent and correct.
#[test]
fn test_unwind_break_then_panic() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_break_then_panic.ori"),
        "unwind_break_then_panic",
    );
}

/// Panic at FIRST element during iteration — iterator is live but no
/// elements have been consumed yet. Verifies cleanup handles zero-consumed
/// iterator state correctly.
#[test]
fn test_unwind_panic_at_first_element() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_panic_at_first_element.ori"),
        "unwind_panic_at_first_element",
    );
}

/// Repeated catch/panic cycles on the same list — stresses RC balance
/// across multiple unwind/recovery sequences.
#[test]
fn test_unwind_repeated_catch_cycles() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_repeated_catch_cycles.ori"),
        "unwind_repeated_catch_cycles",
    );
}

/// Non-iterator local heap value in callee + panic — verifies general
/// RC cleanup for non-iterator heap variables on unwind path.
#[test]
fn test_unwind_callee_local_heap_value() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/unwind/unwind_callee_local_heap_value.ori"),
        "unwind_callee_local_heap_value",
    );
}
