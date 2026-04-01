//! Recursion AOT Tests
//!
//! Tests for direct recursion, mutual recursion, recursive patterns
//! with data structures, recursion with accumulators, and recursive
//! control flow patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// Basic recursion

#[test]
fn test_rec_factorial() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_factorial.ori"),
        "rec_factorial",
    );
}

#[test]
fn test_rec_fibonacci() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_fibonacci.ori"),
        "rec_fibonacci",
    );
}

#[test]
fn test_rec_sum_to() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_sum_to.ori"),
        "rec_sum_to",
    );
}

#[test]
fn test_rec_power() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_power.ori"),
        "rec_power",
    );
}

#[test]
fn test_rec_gcd() {
    assert_aot_success(include_str!("fixtures/recursion/rec_gcd.ori"), "rec_gcd");
}

// Tail-recursive patterns (accumulator)

#[test]
fn test_rec_factorial_acc() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_factorial_acc.ori"),
        "rec_fact_acc",
    );
}

#[test]
fn test_rec_sum_acc() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_sum_acc.ori"),
        "rec_sum_acc",
    );
}

#[test]
fn test_rec_count_digits() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_count_digits.ori"),
        "rec_count_digits",
    );
}

// Mutual recursion

#[test]
fn test_mutual_recursion_not_lowered_moderate_depth() {
    // Mutual recursion is NOT loop-lowered — it uses normal calls.
    // Moderate depth (1000) to verify correctness without stack overflow.
    assert_aot_success(
        include_str!("fixtures/recursion/mutual_recursion_not_lowered_moderate_depth.ori"),
        "mutual_rec_moderate_depth",
    );
}

#[test]
fn test_rec_is_even_odd() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_is_even_odd.ori"),
        "rec_even_odd",
    );
}

#[test]
fn test_rec_mutual_countdown() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_mutual_countdown.ori"),
        "rec_mutual_countdown",
    );
}

// Recursion with Result

#[test]
fn test_rec_safe_divide() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_safe_divide.ori"),
        "rec_safe_divide",
    );
}

// Recursion with match

#[test]
fn test_rec_match_countdown() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_match_countdown.ori"),
        "rec_match_countdown",
    );
}

#[test]
fn test_rec_match_collatz() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_match_collatz.ori"),
        "rec_collatz",
    );
}

// Recursion depth

#[test]
fn test_rec_depth_100() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_depth_100.ori"),
        "rec_depth_100",
    );
}

#[test]
fn test_rec_depth_1000() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_depth_1000.ori"),
        "rec_depth_1000",
    );
}

// Recursion with structs

#[test]
fn test_rec_struct_param() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_struct_param.ori"),
        "rec_struct_param",
    );
}

// Recursive computation patterns

#[test]
fn test_rec_binary_search() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_binary_search.ori"),
        "rec_binary_search",
    );
}

#[test]
fn test_rec_ackermann() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_ackermann.ori"),
        "rec_ackermann",
    );
}

#[test]
fn test_rec_tower_of_hanoi_count() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_tower_of_hanoi_count.ori"),
        "rec_hanoi",
    );
}

// Deep tail recursion (TCO stress tests)

#[test]
fn test_tail_rec_gcd_correct() {
    // gcd is tail-recursive — loop lowering should handle it.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_gcd_correct.ori"),
        "tail_rec_gcd",
    );
}

#[test]
fn test_tail_rec_countdown_deep() {
    // Tail-recursive countdown at depth 200,000. Without TCO this would
    // stack overflow; with loop lowering it runs in O(1) stack space.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_countdown_deep.ori"),
        "tail_rec_countdown_deep",
    );
}

#[test]
fn test_tail_rec_collatz_deep() {
    // Collatz sequence is tail-recursive. Large starting values can
    // produce long sequences. n=837799 takes 524 steps.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_collatz_deep.ori"),
        "tail_rec_collatz_deep",
    );
}

#[test]
fn test_tail_rec_if_else_both_branches() {
    // Both branches are tail calls: f(a) or f(b).
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_if_else_both_branches.ori"),
        "tail_rec_both_branches",
    );
}

#[test]
fn test_tail_rec_factorial_acc_deep() {
    // Tail-recursive factorial with accumulator, deep enough to stress TCO.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_factorial_acc_deep.ori"),
        "tail_rec_fact_deep",
    );
}

// Tail recursion: `recurse()` pattern

#[test]
fn test_tail_rec_recurse_pattern() {
    // `recurse(condition:, base:, step:)` with `self(...)` calls.
    // The __recurse sentinel is resolved to the actual function name,
    // enabling TCO loop lowering.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_recurse_pattern.ori"),
        "tail_rec_recurse_pattern",
    );
}

#[test]
fn test_tail_rec_recurse_deep() {
    // `recurse()` pattern at depth 200,000 — would stack overflow
    // without TCO loop lowering.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_recurse_deep.ori"),
        "tail_rec_recurse_deep",
    );
}

// Tail recursion: RC-managed args

#[test]
fn test_tail_rec_with_list_param() {
    // Tail-recursive function passing an RC-managed list through 100,000
    // iterations. Without TCO this would stack overflow. The list is
    // allocated once and freed once — no leaks, no double-free.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_with_list_param.ori"),
        "tail_rec_list_param",
    );
}

#[test]
fn test_tail_rec_with_string_param() {
    // Tail-recursive function passing an RC-managed string through
    // iterations. Verifies string RC ops are balanced across TCO.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_with_string_param.ori"),
        "tail_rec_string_param",
    );
}

// Tail recursion: mixed tail/non-tail

#[test]
fn test_tail_rec_mixed_tail_and_nontail() {
    // One branch is a tail call, another is non-tail (result + 1).
    // Only the tail call branch should be loop-lowered.
    assert_aot_success(
        include_str!("fixtures/recursion/tail_rec_mixed_tail_and_nontail.ori"),
        "tail_rec_mixed",
    );
}

// Recursion with Option

#[test]
fn test_rec_find_first_above() {
    assert_aot_success(
        include_str!("fixtures/recursion/rec_find_first_above.ori"),
        "rec_find_above",
    );
}
