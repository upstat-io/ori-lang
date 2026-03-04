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
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..5 do sum = sum + i;
    if sum == 10 then 0 else 1
}
"#,
        "for_range_sum",
    );
}

#[test]
fn test_for_range_inclusive() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..=5 do sum = sum + i;
    if sum == 15 then 0 else 1
}
"#,
        "for_range_inclusive",
    );
}

#[test]
fn test_for_range_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in 5..0 do count = count + 1;
    if count == 0 then 0 else 1
}
"#,
        "for_range_empty",
    );
}

#[test]
fn test_for_range_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for i in 0..4 yield i * i;
    if result.length() == 4 then 0 else 1
}
"#,
        "for_range_yield",
    );
}

#[test]
fn test_for_range_with_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..10 if i % 2 == 0 do sum = sum + i;
    if sum == 20 then 0 else 1
}
"#,
        "for_range_with_guard",
    );
}

// -----------------------------------------------------------------------
// List for-loops (regression)
// -----------------------------------------------------------------------

#[test]
fn test_for_list_sum() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for x in [10, 20, 30] do sum = sum + x;
    if sum == 60 then 0 else 1
}
"#,
        "for_list_sum",
    );
}

#[test]
fn test_for_list_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let doubled = for x in [1, 2, 3] yield x * 2;
    if doubled.length() == 3 then 0 else 1
}
"#,
        "for_list_yield",
    );
}

#[test]
fn test_for_list_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let empty: [int] = [];
    for x in empty do count = count + 1;
    if count == 0 then 0 else 1
}
"#,
        "for_list_empty",
    );
}

#[test]
fn test_for_list_with_guard() {
    assert_aot_success(
        r#"
@main () -> int = {
    let evens = for x in [1, 2, 3, 4, 5, 6] if x % 2 == 0 yield x;
    if evens.length() == 3 then 0 else 1
}
"#,
        "for_list_with_guard",
    );
}

// -----------------------------------------------------------------------
// String for-loops (new — character iteration)
// -----------------------------------------------------------------------

#[test]
fn test_for_str_count_chars() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for c in "hello" do count = count + 1;
    if count == 5 then 0 else 1
}
"#,
        "for_str_count_chars",
    );
}

#[test]
fn test_for_str_empty() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for c in "" do count = count + 1;
    if count == 0 then 0 else 1
}
"#,
        "for_str_empty",
    );
}

#[test]
fn test_for_str_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let chars = for c in "abc" yield 1;
    if chars.length() == 3 then 0 else 1
}
"#,
        "for_str_yield",
    );
}

// -----------------------------------------------------------------------
// Option for-loops (new — 0-or-1 element iteration)
// -----------------------------------------------------------------------

#[test]
fn test_for_option_some() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for x in Some(42) do sum = sum + x;
    if sum == 42 then 0 else 1
}
"#,
        "for_option_some",
    );
}

#[test]
fn test_for_option_none() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    let empty: Option<int> = None;
    for x in empty do count = count + 1;
    if count == 0 then 0 else 1
}
"#,
        "for_option_none",
    );
}

#[test]
fn test_for_option_yield_some() {
    assert_aot_success(
        r#"
@main () -> int = {
    let result = for x in Some(5) yield x * 2;
    if result.length() == 1 then 0 else 1
}
"#,
        "for_option_yield_some",
    );
}

#[test]
fn test_for_option_yield_none() {
    assert_aot_success(
        r#"
@main () -> int = {
    let empty: Option<int> = None;
    let result = for x in empty yield x * 2;
    if result.length() == 0 then 0 else 1
}
"#,
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
        r#"
@main () -> int = {
    let sum = 0;
    for c in "ABC" do sum = sum + c.to_int();
    if sum == 198 then 0 else 1
}
"#,
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
        r#"
@main () -> int = {
    let sum = 0;
    for entry in {"a": 10, "b": 20, "c": 30} do sum = sum + entry.1;
    if sum == 60 then 0 else 1
}
"#,
        "for_map_sum",
    );
}

#[test]
fn test_for_map_yield() {
    assert_aot_success(
        r#"
@main () -> int = {
    let values = for entry in {"x": 1, "y": 2, "z": 3} yield entry.1;
    if values.length() == 3 then 0 else 1
}
"#,
        "for_map_yield",
    );
}

#[test]
fn test_for_map_entries() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for entry in {"a": 1, "b": 2} do count = count + 1;
    if count == 2 then 0 else 1
}
"#,
        "for_map_entries",
    );
}

// -----------------------------------------------------------------------
// Break in for-do with mutable variables
// -----------------------------------------------------------------------

#[test]
fn test_for_range_break_with_mutation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in 0..100 do {
        if i == 10 then break;
        count = count + 1;
    };
    if count == 10 then 0 else 1
}
"#,
        "for_range_break_mutation",
    );
}

#[test]
fn test_for_range_break_multiple_mutations() {
    assert_aot_success(
        r#"
@collatz_steps (n: int) -> int = {
    let steps = 0;
    let val = n;
    for _ in 0..10000 do {
        if val == 1 then break;
        if val % 2 == 0 then val = val / 2
        else val = val * 3 + 1;
        steps = steps + 1;
    };
    steps
}

@main () -> int = {
    let result = collatz_steps(n: 27);
    if result == 111 then 0 else 1
}
"#,
        "for_range_break_collatz",
    );
}

#[test]
fn test_for_iter_break_with_mutation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let xs = for i in 0..20 yield i;
    let sum = 0;
    for x in xs do {
        if x >= 5 then break;
        sum = sum + x;
    };
    // 0 + 1 + 2 + 3 + 4 = 10
    if sum == 10 then 0 else 1
}
"#,
        "for_iter_break_mutation",
    );
}

#[test]
fn test_for_range_continue_with_mutation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..10 do {
        if i % 2 == 0 then continue;
        sum = sum + i;
    };
    // 1 + 3 + 5 + 7 + 9 = 25
    if sum == 25 then 0 else 1
}
"#,
        "for_range_continue_mutation",
    );
}

// ── M9 edge cases: inclusive range overflow and step direction ──────

#[test]
fn test_for_range_inclusive_single_element() {
    // Edge case: 0..=0 should iterate exactly once
    assert_aot_success(
        r#"
@main () -> int = {
    let count = 0;
    for i in 0..=0 do count = count + 1;
    if count == 1 then 0 else 1
}
"#,
        "for_range_inclusive_single",
    );
}

#[test]
fn test_for_range_inclusive_with_step() {
    // Inclusive range with step: 0..=10 by 2 → 0, 2, 4, 6, 8, 10
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..=10 by 2 do sum = sum + i;
    // 0 + 2 + 4 + 6 + 8 + 10 = 30
    if sum == 30 then 0 else 1
}
"#,
        "for_range_inclusive_step",
    );
}

#[test]
fn test_for_range_descending_inclusive() {
    // Descending inclusive: 10..=0 by -1 → 10, 9, 8, ..., 0
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 10..=0 by -1 do sum = sum + i;
    // 10 + 9 + ... + 0 = 55
    if sum == 55 then 0 else 1
}
"#,
        "for_range_descending_inclusive",
    );
}

#[test]
fn test_for_range_descending_exclusive() {
    // Descending exclusive: 5..0 by -1 → 5, 4, 3, 2, 1
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 5..0 by -1 do sum = sum + i;
    // 5 + 4 + 3 + 2 + 1 = 15
    if sum == 15 then 0 else 1
}
"#,
        "for_range_descending_exclusive",
    );
}

#[test]
fn test_for_range_with_step_ascending() {
    // Ascending with step: 0..10 by 3 → 0, 3, 6, 9
    assert_aot_success(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..10 by 3 do sum = sum + i;
    // 0 + 3 + 6 + 9 = 18
    if sum == 18 then 0 else 1
}
"#,
        "for_range_step_ascending",
    );
}

#[test]
fn test_for_range_variable_step_inclusive() {
    // Variable step: step value from a function call (not compile-time constant)
    assert_aot_success(
        r#"
@make_step () -> int = 2;

@main () -> int = {
    let $step = make_step();
    let sum = 0;
    for i in 0..=10 by step do sum = sum + i;
    // 0 + 2 + 4 + 6 + 8 + 10 = 30
    if sum == 30 then 0 else 1
}
"#,
        "for_range_variable_step",
    );
}

// ── Zero step panics ─────────────────────────────────────────────────

#[test]
fn test_for_range_zero_step_panics_exclusive() {
    // Zero step on exclusive range should panic, not infinite-loop.
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..10 by 0 do sum = sum + i;
    0
}
"#,
    );
    assert_ne!(exit_code, 0, "zero step should panic (non-zero exit)");
    assert!(
        stderr.contains("range step cannot be zero"),
        "stderr should contain panic message, got: {stderr}"
    );
}

#[test]
fn test_for_range_zero_step_panics_inclusive() {
    // Zero step on inclusive range should panic.
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    let sum = 0;
    for i in 0..=10 by 0 do sum = sum + i;
    0
}
"#,
    );
    assert_ne!(exit_code, 0, "zero step should panic (non-zero exit)");
    assert!(
        stderr.contains("range step cannot be zero"),
        "stderr should contain panic message, got: {stderr}"
    );
}

#[test]
fn test_for_range_zero_step_panics_runtime() {
    // Zero step from a runtime variable should also panic.
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@zero () -> int = 0;

@main () -> int = {
    let sum = 0;
    for i in 0..=10 by zero() do sum = sum + i;
    0
}
"#,
    );
    assert_ne!(exit_code, 0, "runtime zero step should panic");
    assert!(
        stderr.contains("range step cannot be zero"),
        "stderr should contain panic message, got: {stderr}"
    );
}
