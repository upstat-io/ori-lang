//! TRMC (Tail Recursion Modulo Context) AOT behavioral tests.
//!
//! Section 13.8 Matrix G — end-to-end Ori source → ARC lowering → TRMC
//! rewrite → AIMS analysis → LLVM emission → execution → correct output.
//!
//! These tests verify the TRMC rewrite produces semantically correct code,
//! not just structurally valid IR. They cover the concrete fixtures from
//! Section 13.8 and cross-system interaction axes from Matrix E/F.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// Concrete fixture: trmc_list_map_increment (D1, D3, D4, D6, D7, E1, E11)
// Single-arg recursion changing arg. Recursive list map: increment each element.

#[test]
fn test_trmc_list_map_increment() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_list_map_increment.ori"),
        "trmc_list_map_increment",
    );
}

// Concrete fixture: trmc_list_map_two_args (D2, F: arg count=2, change=some)
// 2-arg recursion where only the second arg changes.

#[test]
fn test_trmc_list_map_two_args() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_list_map_two_args.ori"),
        "trmc_list_map_two_args",
    );
}

// Concrete fixture: trmc_tree_mirror (D8, D9, F: hole=0 and 1)
// Binary tree mirror: tests hole at different field positions.

#[test]
fn test_trmc_tree_mirror() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_tree_mirror.ori"),
        "trmc_tree_mirror",
    );
}

// Concrete fixture: trmc_base_case_immediate (D7, F: depth=0)
// Call with base case (Nil) — 0 recursive iterations.

#[test]
fn test_trmc_base_case_immediate() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_base_case_immediate.ori"),
        "trmc_base_case_immediate",
    );
}

// Concrete fixture: trmc_multi_field_ctor (D9, F: ctor arity=3, hole=last)
// 3-field constructor with hole at the last field.
// Uses int fields only — str fields in recursive enums hit a pre-existing
// misalignment bug (enum payload layout for multi-word + boxed pointer).

#[test]
fn test_trmc_multi_field_ctor() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_multi_field_ctor.ori"),
        "trmc_multi_field_ctor",
    );
}

// Concrete fixture: trmc_deep_recursion (F: depth=100+)
// Build a list of 100+ elements via recursion. Without TRMC optimization,
// deep recursion patterns benefit from constant stack usage.

#[test]
fn test_trmc_deep_recursion() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_deep_recursion.ori"),
        "trmc_deep_recursion",
    );
}

// Concrete fixture: trmc_enum_with_tag_change
// Recursive enum where constructor variant changes between iterations.

#[test]
fn test_trmc_enum_with_tag_change() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_enum_with_tag_change.ori"),
        "trmc_enum_with_tag_change",
    );
}

// Matrix E interaction: TRMC × RC emission (E1)
// Context vars must have correct RC ops — no double-free on context root.

#[test]
fn test_trmc_rc_balance() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_rc_balance.ori"),
        "trmc_rc_balance",
    );
}

// Matrix E interaction: TRMC × FIP contract (E5)
// Rewritten function with alloc-balanced pattern match should potentially
// get FipContract upgrade.

#[test]
fn test_trmc_reuse_pattern() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_reuse_pattern.ori"),
        "trmc_reuse_pattern",
    );
}

// Multiple base cases across match arms.

#[test]
fn test_trmc_multiple_base_cases() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_multiple_base_cases.ori"),
        "trmc_multiple_base_cases",
    );
}

// Shared recursive data structure — RC must handle sharing correctly.

#[test]
fn test_trmc_shared_structure() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_shared_structure.ori"),
        "trmc_shared_structure",
    );
}

// Interpreter vs AOT parity — same output from both backends.
// (This is covered by dual-exec-verify.sh but having an inline test
// catches regressions faster.)

#[test]
fn test_trmc_nested_recursion() {
    assert_aot_success(
        include_str!("fixtures/trmc/trmc_nested_recursion.ori"),
        "trmc_nested_recursion",
    );
}
