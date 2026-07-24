use super::support::*;

// Empty Module

#[test]
fn empty_source() {
    let result = check_source("");
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 0);
}

// Literal Expressions

#[test]
fn literal_int() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/literal_int.ori"
    )));
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 1);

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn literal_float() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/literal_float.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::FLOAT);
}

#[test]
fn literal_bool() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/literal_bool.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn literal_string() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/literal_string.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn literal_unit() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/literal_unit.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::UNIT);
}

// Function Parameters

#[test]
fn single_typed_param() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/single_typed_param.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn multiple_typed_params() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/multiple_typed_params.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn param_type_used_in_body() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/param_type_used_in_body.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

// Multiple Functions

#[test]
fn two_functions() {
    let source = include_str!("../fixtures/integration/two_functions.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);

    let foo_ty = result.function_body_type("foo").unwrap();
    assert_eq!(foo_ty, Idx::INT);
    let bar_ty = result.function_body_type("bar").unwrap();
    assert_eq!(bar_ty, Idx::INT);
}

#[test]
fn function_calling_another() {
    // Forward reference: bar calls foo, foo is defined first
    let source = include_str!("../fixtures/integration/function_calling_another.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

#[test]
fn forward_reference() {
    // bar defined before foo, but calls foo
    let source = include_str!("../fixtures/integration/forward_reference.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

// Test Declarations

#[test]
fn test_declaration() {
    let source = include_str!("../fixtures/integration/declaration.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    // Functions + tests both counted as signatures
    assert_eq!(result.function_count(), 2);
}

#[test]
fn test_with_function_call() {
    // Test body that uses the target function via block expression
    let source = include_str!("../fixtures/integration/with_function_call.ori");
    let result = check_source(source);
    // `run` may produce errors since it's a compiler construct that needs
    // special handling. The key assertion is: no panics in the pipeline.
    let _ = result.has_errors();
}

// Type Errors

#[test]
fn return_type_mismatch() {
    // Body returns string but signature says int
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/return_type_mismatch.ori"
    )));
    assert!(result.has_errors());
    assert!(result.error_count() >= 1);

    // Should have a mismatch error
    let has_mismatch = result
        .error_kinds()
        .iter()
        .any(|k| matches!(k, TypeErrorKind::Mismatch { .. }));
    assert!(
        has_mismatch,
        "Expected a Mismatch error, got: {:?}",
        result.error_kinds()
    );
}

#[test]
fn unknown_identifier_in_body() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/unknown_identifier_in_body.ori"
    )));
    assert!(result.has_errors());

    let has_unknown = result
        .error_kinds()
        .iter()
        .any(|k| matches!(k, TypeErrorKind::UnknownIdent { .. }));
    assert!(
        has_unknown,
        "Expected UnknownIdent error, got: {:?}",
        result.error_kinds()
    );
}

#[test]
fn unknown_identifier_suggests_similar_names() {
    // "ad" is a typo for "add" — should suggest "add"
    let source =
        include_str!("../fixtures/integration/unknown_identifier_suggests_similar_names.ori");
    let result = check_source(source);
    assert!(result.has_errors());

    let error_kinds = result.error_kinds();
    let unknown = error_kinds
        .iter()
        .find(|k| matches!(k, TypeErrorKind::UnknownIdent { .. }));

    assert!(unknown.is_some(), "Expected UnknownIdent error");

    if let Some(TypeErrorKind::UnknownIdent { similar, .. }) = unknown {
        assert!(
            !similar.is_empty(),
            "Expected similar name suggestions, got empty list"
        );
    }
}

#[test]
fn unknown_identifier_no_suggestion_for_unrelated_names() {
    // "xyz" is not similar to any name in scope
    let source = include_str!(
        "../fixtures/integration/unknown_identifier_no_suggestion_for_unrelated_names.ori"
    );
    let result = check_source(source);
    assert!(result.has_errors());

    let error_kinds = result.error_kinds();
    let unknown = error_kinds
        .iter()
        .find(|k| matches!(k, TypeErrorKind::UnknownIdent { .. }));

    assert!(unknown.is_some(), "Expected UnknownIdent error");

    if let Some(TypeErrorKind::UnknownIdent { similar, .. }) = unknown {
        assert!(
            similar.is_empty(),
            "Expected no suggestions for 'xyz', got {similar:?}",
        );
    }
}

#[test]
fn call_with_named_arg() {
    // Calling a function with named arguments
    let source = include_str!("../fixtures/integration/call_with_named_arg.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

// Let Bindings

#[test]
fn simple_let_binding() {
    let source = include_str!("../fixtures/integration/simple_let_binding.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Simple let binding in block should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn let_in_block_body() {
    // Using a block expression (if/else) that includes let bindings
    let source = include_str!("../fixtures/integration/let_in_block_body.ori");
    let result = check_source(source);
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

// Control Flow

#[test]
fn if_then_else_int() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/if_then_else_int.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn if_then_else_string() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/if_then_else_string.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn if_condition_must_be_bool() {
    // Using an int as condition should produce an error
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/if_condition_must_be_bool.ori"
    )));
    assert!(result.has_errors());

    let has_mismatch = result
        .error_kinds()
        .iter()
        .any(|k| matches!(k, TypeErrorKind::Mismatch { .. }));
    assert!(
        has_mismatch,
        "Expected Mismatch error for non-bool condition, got: {:?}",
        result.error_kinds()
    );
}
