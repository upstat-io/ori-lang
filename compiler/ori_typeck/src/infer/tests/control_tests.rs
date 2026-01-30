#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tests for control.rs - control flow type inference.

use super::check_source;

#[test]
fn test_if_expression_type() {
    let result = check_source(
        r#"
        @main () -> int = if true then 1 else 2
    "#,
    );

    assert!(!result.has_errors(), "If expression should type check");
}

#[test]
fn test_if_branch_type_mismatch() {
    let result = check_source(
        r#"
        @main () -> int = if true then 1 else "hello"
    "#,
    );

    assert!(result.has_errors(), "Should error on branch type mismatch");
}

#[test]
fn test_for_loop_over_list() {
    let result = check_source(
        r#"
        @main () -> void = for x in [1, 2, 3] do ()
    "#,
    );

    assert!(!result.has_errors(), "For loop over list should work");
}

#[test]
fn test_for_loop_over_non_iterable() {
    let result = check_source(
        r#"
        @main () -> void = for x in 42 do ()
    "#,
    );

    assert!(result.has_errors(), "Should error on non-iterable");
    assert!(
        result.has_error_containing("iterable")
            || result.has_error_containing("List")
            || result.has_error_containing("cannot iterate"),
        "Error should mention iterability"
    );
}

#[test]
fn test_for_yield_collects_to_list() {
    let result = check_source(
        r#"
        @main () = for x in [1, 2, 3] yield x * 2
    "#,
    );

    assert!(!result.has_errors(), "For yield should produce list");
}

#[test]
fn test_match_exhaustiveness() {
    let result = check_source(
        r#"
        @main () -> int = match(
            Some(42),
            Some(x) -> x,
            None -> 0
        )
    "#,
    );

    assert!(!result.has_errors(), "Exhaustive match should work");
}

#[test]
fn test_loop_with_break() {
    let result = check_source(
        r#"
        @main () -> int = loop(
            break 42
        )
    "#,
    );

    assert!(!result.has_errors(), "Loop with break value should work");
}

#[test]
fn test_block_expression() {
    let result = check_source(
        r#"
        @main () -> int = run(
            let x = 1,
            let y = 2,
            x + y
        )
    "#,
    );

    assert!(!result.has_errors(), "Block expression should work");
}

#[test]
fn test_if_condition_must_be_bool() {
    let result = check_source(
        r#"
        @main () -> int = if 42 then 1 else 2
    "#,
    );

    assert!(result.has_errors(), "If condition must be bool");
    assert!(
        result.has_error_containing("bool") || result.has_error_containing("type mismatch"),
        "Error should mention bool requirement"
    );
}
