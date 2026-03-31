//! Variable Scoping & Block Expression AOT Tests
//!
//! Tests for let bindings, variable shadowing, block expressions as values,
//! nested scopes, if/match as expression in value position, and scope
//! interaction with control flow.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Basic let bindings ───

#[test]
fn test_scope_let_basic() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_let_basic.ori"),
        "scope_let_basic",
    );
}

#[test]
fn test_scope_let_type_annotation() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_let_type_annotation.ori"),
        "scope_let_type_ann",
    );
}

#[test]
fn test_scope_let_chain() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_let_chain.ori"),
        "scope_let_chain",
    );
}

// ─── Variable shadowing ───

#[test]
fn test_scope_shadow_same_type() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_shadow_same_type.ori"),
        "scope_shadow_same",
    );
}

#[test]
fn test_scope_shadow_different_type() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_shadow_different_type.ori"),
        "scope_shadow_diff_type",
    );
}

#[test]
fn test_scope_shadow_uses_previous() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_shadow_uses_previous.ori"),
        "scope_shadow_uses_prev",
    );
}

#[test]
fn test_scope_shadow_in_nested_block() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_shadow_in_nested_block.ori"),
        "scope_shadow_nested",
    );
}

#[test]
fn test_scope_shadow_three_levels() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_shadow_three_levels.ori"),
        "scope_shadow_three",
    );
}

// ─── Block expressions as values ───

#[test]
fn test_scope_block_as_value() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_block_as_value.ori"),
        "scope_block_value",
    );
}

#[test]
fn test_scope_block_single_expression() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_block_single_expression.ori"),
        "scope_block_single",
    );
}

#[test]
fn test_scope_nested_blocks_as_values() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_nested_blocks_as_values.ori"),
        "scope_nested_blocks",
    );
}

#[test]
fn test_scope_block_with_side_effects() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_block_with_side_effects.ori"),
        "scope_block_side_effects",
    );
}

// ─── If-else as expression ───

#[test]
fn test_scope_if_else_value() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_if_else_value.ori"),
        "scope_if_value",
    );
}

#[test]
fn test_scope_if_else_computed() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_if_else_computed.ori"),
        "scope_if_computed",
    );
}

#[test]
fn test_scope_nested_if_expression() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_nested_if_expression.ori"),
        "scope_nested_if_expr",
    );
}

#[test]
fn test_scope_if_block_branches() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_if_block_branches.ori"),
        "scope_if_block_branches",
    );
}

// ─── Match as expression ───

#[test]
fn test_scope_match_expression_value() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_match_expression_value.ori"),
        "scope_match_value",
    );
}

#[test]
fn test_scope_match_in_let() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_match_in_let.ori"),
        "scope_match_in_let",
    );
}

#[test]
fn test_scope_match_block_arms() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_match_block_arms.ori"),
        "scope_match_block_arms",
    );
}

// ─── Expressions in complex positions ───

#[test]
fn test_scope_expression_in_function_arg() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_expression_in_function_arg.ori"),
        "scope_expr_in_arg",
    );
}

#[test]
fn test_scope_expression_in_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_expression_in_arithmetic.ori"),
        "scope_expr_in_arith",
    );
}

#[test]
fn test_scope_expression_in_comparison() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_expression_in_comparison.ori"),
        "scope_expr_in_cmp",
    );
}

// ─── Scope interaction with control flow ───

#[test]
fn test_scope_shadow_in_loop() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_shadow_in_loop.ori"),
        "scope_shadow_loop",
    );
}

#[test]
fn test_scope_let_in_match_arm() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_let_in_match_arm.ori"),
        "scope_let_in_match",
    );
}

#[test]
fn test_scope_block_in_loop_body() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_block_in_loop_body.ori"),
        "scope_block_in_loop",
    );
}

// ─── Complex scoping patterns ───

#[test]
fn test_scope_closure_captures_outer() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_closure_captures_outer.ori"),
        "scope_closure_captures",
    );
}

#[test]
fn test_scope_shadow_before_closure() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_shadow_before_closure.ori"),
        "scope_shadow_before_closure",
    );
}

#[test]
fn test_scope_many_lets_same_name() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_many_lets_same_name.ori"),
        "scope_many_shadows",
    );
}

#[test]
fn test_scope_string_shadow() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_string_shadow.ori"),
        "scope_string_shadow",
    );
}

#[test]
fn test_scope_tuple_destructure() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_tuple_destructure.ori"),
        "scope_tuple_destr",
    );
}

#[test]
fn test_scope_if_else_string_value() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_if_else_string_value.ori"),
        "scope_if_str_value",
    );
}

#[test]
fn test_scope_block_returning_struct() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_block_returning_struct.ori"),
        "scope_block_struct",
    );
}

#[test]
fn test_scope_let_in_each_branch() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_let_in_each_branch.ori"),
        "scope_let_each_branch",
    );
}

// ─── Match with various value types ───

#[test]
fn test_scope_match_bool_expression() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_match_bool_expression.ori"),
        "scope_match_bool",
    );
}

#[test]
fn test_scope_match_nested_in_if() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_match_nested_in_if.ori"),
        "scope_match_in_if",
    );
}

#[test]
fn test_scope_if_in_match_arm() {
    assert_aot_success(
        include_str!("fixtures/scoping/scope_if_in_match_arm.ori"),
        "scope_if_in_match",
    );
}
