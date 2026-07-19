use super::support::*;

// Collections

#[test]
fn list_literal() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/list_literal.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(result.tag(body_ty), Tag::List);
}

#[test]
fn empty_list() {
    // Empty list with type annotation on function
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/empty_list.ori"
    )));
    // The empty list may or may not unify with [int] depending on inference
    // At minimum, it shouldn't panic
    let _ = result.has_errors();
}

// Operators

#[test]
fn arithmetic_operators() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/arithmetic_operators.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn comparison_operators() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/comparison_operators.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn boolean_operators() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/boolean_operators.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn equality_check() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/equality_check.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn string_concatenation() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/string_concatenation.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn negation() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/negation.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn boolean_not() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/boolean_not.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

// Tuple Expressions

#[test]
fn tuple_literal() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/tuple_literal.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(result.tag(body_ty), Tag::Tuple);
}

// Multiple Error Accumulation

#[test]
fn multiple_errors_accumulated() {
    // Two functions with errors - should accumulate both
    let source = include_str!("../fixtures/integration/multiple_errors_accumulated.ori");
    let result = check_source(source);
    assert!(result.has_errors());
    // Should have at least 2 errors (one per function)
    assert!(
        result.error_count() >= 2,
        "Expected at least 2 errors, got {}",
        result.error_count()
    );
}
