//! Tests for call.rs - function and method call type inference.

use super::check_source;

#[test]
fn test_function_call_named_args() {
    let result = check_source(
        r#"
        @add (a: int, b: int) -> int = a + b
        @main () -> int = @add(a: 1, b: 2)
    "#,
    );

    assert!(!result.has_errors(), "Named arg call should succeed");
}

#[test]
fn test_lambda_call_type_mismatch() {
    let result = check_source(
        r#"
        @main () -> int = run(
            let add = (a: int, b: int) -> a + b,
            add(a: "hello", b: 2)
        )
    "#,
    );

    // Type mismatch in lambda call should produce an error
    assert!(
        result.has_errors(),
        "Should error on type mismatch: {:?}",
        result.typed.errors
    );
}

#[test]
fn test_method_call_on_list() {
    let result = check_source(
        r#"
        @main () -> int = [1, 2, 3].len()
    "#,
    );

    assert!(!result.has_errors(), "List.len() should work");
}

#[test]
fn test_method_call_with_named_args() {
    let result = check_source(
        r#"
        @main () = [1, 2, 3].map(transform: (x: int) -> x * 2)
    "#,
    );

    // Named arguments work for method calls
    assert!(
        !result.has_errors(),
        "Named arg method call should work: {:?}",
        result.typed.errors
    );
}

#[test]
fn test_capability_propagation_missing() {
    let result = check_source(
        r#"
        @fetch (url: str) -> str uses Http = "response"
        @main () -> str = @fetch(url: "http://example.com")
    "#,
    );

    assert!(result.has_errors(), "Should error on missing capability");
    assert!(
        result.has_error_containing("capability")
            || result.has_error_containing("Http")
            || result.has_error_containing("uses"),
        "Error should mention capability"
    );
}

#[test]
fn test_builtin_method_on_option() {
    let result = check_source(
        r#"
        @main () -> int = Some(42).unwrap_or(default: 0)
    "#,
    );

    assert!(!result.has_errors(), "Option.unwrap_or should work");
}

#[test]
fn test_method_not_found() {
    let result = check_source(
        r#"
        @main () -> int = 42.nonexistent()
    "#,
    );

    assert!(result.has_errors(), "Should error on unknown method");
    assert!(
        result.has_error_containing("no method") || result.has_error_containing("nonexistent"),
        "Error should mention the method"
    );
}
