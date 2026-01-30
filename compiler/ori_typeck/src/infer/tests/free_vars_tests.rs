#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tests for `free_vars.rs` - free variable collection for closure self-capture detection.

use super::check_source;

#[test]
fn test_closure_self_capture_detected() {
    let result = check_source(
        r#"
        @main () -> void = run(
            let f = () -> f,
            ()
        )
    "#,
    );

    assert!(result.has_errors(), "Self-capture should be detected");
    assert!(
        result.has_error_containing("capture itself"),
        "Error should mention self-capture"
    );
}

#[test]
fn test_closure_captures_outer_binding() {
    let result = check_source(
        r#"
        @main () -> int = run(
            let x = 42,
            let f = () -> x,
            f()
        )
    "#,
    );

    // Capturing outer bindings is fine, not self-capture
    assert!(
        !result.has_error_containing("capture itself"),
        "Outer binding capture should be allowed"
    );
}

#[test]
fn test_closure_self_capture_in_call() {
    let result = check_source(
        r#"
        @main () -> void = run(
            let f = (x: int) -> f(x: x + 1),
            ()
        )
    "#,
    );

    assert!(
        result.has_errors(),
        "Self-capture in call should be detected"
    );
    assert!(
        result.has_error_containing("capture itself"),
        "Error should mention self-capture"
    );
}

#[test]
fn test_closure_self_capture_in_branch() {
    let result = check_source(
        r#"
        @main () -> void = run(
            let f = () -> if true then f else f,
            ()
        )
    "#,
    );

    assert!(
        result.has_errors(),
        "Self-capture in branch should be detected"
    );
}

#[test]
fn test_shadowing_allows_reuse() {
    // If f is shadowed by the lambda parameter, it's not self-capture
    // (though this particular example may still error for other reasons)
    let result = check_source(
        r#"
        @main () -> int = run(
            let f = 42,
            let g = (f: int) -> f,
            g(f: 10)
        )
    "#,
    );

    // Should not have self-capture error
    assert!(
        !result.has_error_containing("capture itself"),
        "Shadowed binding should not be self-capture"
    );
}

#[test]
fn test_nested_lambda_capture() {
    let result = check_source(
        r#"
        @main () -> int = run(
            let x = 1,
            let f = () -> run(
                let g = () -> x,
                g()
            ),
            f()
        )
    "#,
    );

    // Nested lambdas capturing outer scope is fine
    assert!(
        !result.has_error_containing("capture itself"),
        "Nested capture of outer scope should be allowed"
    );
}

#[test]
fn test_free_vars_in_match_arm() {
    let result = check_source(
        r#"
        @main () -> int = run(
            let outer = 10,
            let opt = Some(5),
            match(opt,
                Some(x) -> x + outer,
                None -> outer
            )
        )
    "#,
    );

    assert!(!result.has_errors(), "Free vars in match arm should work");
}

#[test]
fn test_free_vars_respects_pattern_bindings() {
    let result = check_source(
        r#"
        @main () -> int = run(
            let opt = Some(42),
            match(opt,
                Some(x) -> x,
                None -> 0
            )
        )
    "#,
    );

    // Pattern-bound 'x' should not be considered free
    assert!(!result.has_errors(), "Pattern bindings should be respected");
}
