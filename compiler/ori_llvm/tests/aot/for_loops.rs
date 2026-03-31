//! AOT For-Loop Tests
//!
//! End-to-end tests for for-loops over all iterable types: Range, List, Str,
//! Option, Set, Map. Covers both `do` (side effects) and `yield` (collection)
//! forms, including guards.
//!
//! These are regression tests for Range/List and new coverage for Str/Option/Set/Map.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

// -----------------------------------------------------------------------
// Range for-loops (regression)
// -----------------------------------------------------------------------

#[test]
fn test_for_range_sum() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_sum.ori"),
        "for_range_sum",
    );
}

#[test]
fn test_for_range_inclusive() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_inclusive.ori"),
        "for_range_inclusive",
    );
}

#[test]
fn test_for_range_empty() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_empty.ori"),
        "for_range_empty",
    );
}

#[test]
fn test_for_range_yield() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_yield.ori"),
        "for_range_yield",
    );
}

#[test]
fn test_for_range_with_guard() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_with_guard.ori"),
        "for_range_with_guard",
    );
}

// -----------------------------------------------------------------------
// List for-loops (regression)
// -----------------------------------------------------------------------

#[test]
fn test_for_list_sum() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_list_sum.ori"),
        "for_list_sum",
    );
}

#[test]
fn test_for_list_yield() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_list_yield.ori"),
        "for_list_yield",
    );
}

#[test]
fn test_for_list_empty() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_list_empty.ori"),
        "for_list_empty",
    );
}

#[test]
fn test_for_list_with_guard() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_list_with_guard.ori"),
        "for_list_with_guard",
    );
}

// -----------------------------------------------------------------------
// String for-loops (new — character iteration)
// -----------------------------------------------------------------------

#[test]
fn test_for_str_count_chars() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_str_count_chars.ori"),
        "for_str_count_chars",
    );
}

#[test]
fn test_for_str_empty() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_str_empty.ori"),
        "for_str_empty",
    );
}

#[test]
fn test_for_str_yield() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_str_yield.ori"),
        "for_str_yield",
    );
}

// -----------------------------------------------------------------------
// Option for-loops (new — 0-or-1 element iteration)
// -----------------------------------------------------------------------

#[test]
fn test_for_option_some() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_option_some.ori"),
        "for_option_some",
    );
}

#[test]
fn test_for_option_none() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_option_none.ori"),
        "for_option_none",
    );
}

#[test]
fn test_for_option_yield_some() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_option_yield_some.ori"),
        "for_option_yield_some",
    );
}

#[test]
fn test_for_option_yield_none() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_option_yield_none.ori"),
        "for_option_yield_none",
    );
}

// -----------------------------------------------------------------------
// String for-loops — character value verification
// -----------------------------------------------------------------------

#[test]
fn test_for_str_char_values() {
    // Verify actual codepoint values: 'A'=65, 'B'=66, 'C'=67 → sum=198
    assert_aot_success(
        include_str!("fixtures/for_loops/for_str_char_values.ori"),
        "for_str_char_values",
    );
}

// -----------------------------------------------------------------------
// Set for-loops — blocked: .iter().collect() not yet in AOT codegen.
// lower_for_data_array (Set codepath) is identical to List, so List
// tests provide equivalent coverage. Add Set tests when iterator
// method dispatch is available in AOT.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// Map for-loops (key-value tuple iteration)
// -----------------------------------------------------------------------

#[test]
fn test_for_map_sum() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_map_sum.ori"),
        "for_map_sum",
    );
}

#[test]
fn test_for_map_yield() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_map_yield.ori"),
        "for_map_yield",
    );
}

#[test]
fn test_for_map_entries() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_map_entries.ori"),
        "for_map_entries",
    );
}

// -----------------------------------------------------------------------
// Break in for-do with mutable variables
// -----------------------------------------------------------------------

#[test]
fn test_for_range_break_with_mutation() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_break_with_mutation.ori"),
        "for_range_break_mutation",
    );
}

#[test]
fn test_for_range_break_multiple_mutations() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_break_multiple_mutations.ori"),
        "for_range_break_collatz",
    );
}

#[test]
fn test_for_iter_break_with_mutation() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_iter_break_with_mutation.ori"),
        "for_iter_break_mutation",
    );
}

#[test]
fn test_for_range_continue_with_mutation() {
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_continue_with_mutation.ori"),
        "for_range_continue_mutation",
    );
}

// ── M9 edge cases: inclusive range overflow and step direction ──────

#[test]
fn test_for_range_inclusive_single_element() {
    // Edge case: 0..=0 should iterate exactly once
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_inclusive_single_element.ori"),
        "for_range_inclusive_single",
    );
}

#[test]
fn test_for_range_inclusive_with_step() {
    // Inclusive range with step: 0..=10 by 2 → 0, 2, 4, 6, 8, 10
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_inclusive_with_step.ori"),
        "for_range_inclusive_step",
    );
}

#[test]
fn test_for_range_descending_inclusive() {
    // Descending inclusive: 10..=0 by -1 → 10, 9, 8, ..., 0
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_descending_inclusive.ori"),
        "for_range_descending_inclusive",
    );
}

#[test]
fn test_for_range_descending_exclusive() {
    // Descending exclusive: 5..0 by -1 → 5, 4, 3, 2, 1
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_descending_exclusive.ori"),
        "for_range_descending_exclusive",
    );
}

#[test]
fn test_for_range_with_step_ascending() {
    // Ascending with step: 0..10 by 3 → 0, 3, 6, 9
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_with_step_ascending.ori"),
        "for_range_step_ascending",
    );
}

#[test]
fn test_for_range_variable_step_inclusive() {
    // Variable step: step value from a function call (not compile-time constant)
    assert_aot_success(
        include_str!("fixtures/for_loops/for_range_variable_step_inclusive.ori"),
        "for_range_variable_step",
    );
}

// ── Zero step panics ─────────────────────────────────────────────────

#[test]
fn test_for_range_zero_step_panics_exclusive() {
    // Zero step on exclusive range should panic, not infinite-loop.
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/for_loops/for_range_zero_step_panics_exclusive.ori"
    ));
    assert_ne!(exit_code, 0, "zero step should panic (non-zero exit)");
    assert!(
        stderr.contains("range step cannot be zero"),
        "stderr should contain panic message, got: {stderr}"
    );
}

#[test]
fn test_for_range_zero_step_panics_inclusive() {
    // Zero step on inclusive range should panic.
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/for_loops/for_range_zero_step_panics_inclusive.ori"
    ));
    assert_ne!(exit_code, 0, "zero step should panic (non-zero exit)");
    assert!(
        stderr.contains("range step cannot be zero"),
        "stderr should contain panic message, got: {stderr}"
    );
}

#[test]
fn test_for_range_zero_step_panics_runtime() {
    // Zero step from a runtime variable should also panic.
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/for_loops/for_range_zero_step_panics_runtime.ori"
    ));
    assert_ne!(exit_code, 0, "runtime zero step should panic");
    assert!(
        stderr.contains("range step cannot be zero"),
        "stderr should contain panic message, got: {stderr}"
    );
}
