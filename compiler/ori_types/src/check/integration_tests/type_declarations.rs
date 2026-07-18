use super::support::*;

// Regression Guards

#[test]
fn only_comments() {
    // Source with only comments should be treated as empty
    let result = check_source_allow_parse_errors("// just a comment");
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 0);
}

#[test]
fn function_returning_void() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/function_returning_void.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::UNIT);
}

#[test]
fn many_functions() {
    let source = include_str!("../fixtures/integration/many_functions.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 5);
}

// Type Definition Exports

#[test]
fn struct_type_exported() {
    let source = include_str!("../fixtures/integration/struct_type_exported.ori");
    let result = check_source(source);
    assert!(!result.has_errors());

    // Includes built-in Ordering + user-defined Point
    let types = &result.result.typed.types;
    let point = types.iter().find(|t| {
        let name = result.interner.lookup(t.name);
        name == "Point"
    });
    assert!(point.is_some(), "Point type should be exported");

    if let crate::TypeKind::Struct(ref s) = point.unwrap().kind {
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].ty, Idx::INT);
        assert_eq!(s.fields[1].ty, Idx::INT);
    } else {
        panic!("Expected Struct type kind, got {:?}", point.unwrap().kind);
    }
}

#[test]
fn enum_type_exported() {
    let source = include_str!("../fixtures/integration/enum_type_exported.ori");
    let result = check_source(source);
    assert!(!result.has_errors());

    let types = &result.result.typed.types;
    let color = types.iter().find(|t| {
        let name = result.interner.lookup(t.name);
        name == "Color"
    });
    assert!(color.is_some(), "Color type should be exported");

    if let crate::TypeKind::Enum { ref variants } = color.unwrap().kind {
        assert_eq!(variants.len(), 3);
    } else {
        panic!("Expected Enum type kind, got {:?}", color.unwrap().kind);
    }
}

#[test]
fn builtin_ordering_always_exported() {
    // Even an empty module has the built-in Ordering type registered.
    let result = check_source("");
    let ordering = result.result.typed.types.iter().find(|t| {
        let name = result.interner.lookup(t.name);
        name == "Ordering"
    });
    assert!(
        ordering.is_some(),
        "Built-in Ordering type should always be exported"
    );
    if let crate::TypeKind::Enum { ref variants } = ordering.unwrap().kind {
        assert_eq!(
            variants.len(),
            3,
            "Ordering should have Less, Equal, Greater"
        );
    } else {
        panic!("Ordering should be an enum");
    }
}

// Invalid Return Type Annotations

#[test]
fn bogus_return_type_is_rejected() {
    // `-> garbage` is not a valid type — should produce a type error
    let source = include_str!("../fixtures/integration/bogus_return_type_is_rejected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined return type `garbage`, got none"
    );
}

#[test]
fn bogus_return_type_on_method_is_rejected() {
    // Same bug but on a method with `self` — this is the user's exact repro
    let source =
        include_str!("../fixtures/integration/bogus_return_type_on_method_is_rejected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined return type `garbage` on method, got none"
    );
}

#[test]
fn bogus_return_type_in_impl_block_is_rejected() {
    // BUG: impl block methods silently accept bogus return type annotations.
    // `-> nt` is not a valid type but the type checker accepts it and the
    // program runs, producing correct output with no errors.
    let source =
        include_str!("../fixtures/integration/bogus_return_type_in_impl_block_is_rejected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined return type `nt` in impl block, got none"
    );
}

#[test]
fn bogus_param_type_is_rejected() {
    // Also check parameter types — `garbage` as a param type should error
    let source = include_str!("../fixtures/integration/bogus_param_type_is_rejected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined param type `garbage`, got none"
    );
}

#[test]
fn bogus_return_type_via_imports_api() {
    // Test the exact code path the WASM playground uses:
    // check_module_with_imports with an empty register_fn
    let source =
        include_str!("../fixtures/integration/bogus_return_type_on_method_is_rejected.ori");
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);
    assert!(
        parsed.errors.is_empty(),
        "Parse errors: {:?}",
        parsed.errors
    );

    let (type_result, _pool) =
        crate::check_module_with_imports(&parsed.module, &parsed.arena, &interner, |_checker| {});

    assert!(
        type_result.has_errors(),
        "check_module_with_imports should reject `-> garbage` but produced no errors"
    );
}

#[test]
fn valid_integer_return_annotation_has_no_type_errors() {
    // A valid return annotation produces no type errors.
    let source = include_str!("../fixtures/integration/valid_return_type_still_works.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Valid return type `int` should not produce errors: {:?}",
        result.error_kinds()
    );
}

// Impl Block `self` Parameter — Type Checking

#[test]
fn impl_self_field_access_type_checks() {
    // An impl method's `self` resolves to the impl type and exposes its fields.
    let source = include_str!("../fixtures/integration/impl_self_field_access_type_checks.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Valid impl method with self field access should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_self_with_additional_params() {
    // self and additional typed parameters should all resolve correctly
    let source = include_str!("../fixtures/integration/impl_self_with_additional_params.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Impl methods with self + additional params should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_self_return_type_mismatch_detected() {
    // Body returns int (self.x + self.y), but declared return type is str.
    // The type checker must catch this mismatch.
    let source =
        include_str!("../fixtures/integration/impl_self_return_type_mismatch_detected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Impl method returning int but declared -> str should error"
    );
}

#[test]
fn impl_self_returning_self_type() {
    // Self as return type should resolve to the impl type
    let source = include_str!("../fixtures/integration/impl_self_returning_self_type.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Impl method returning Self should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_associated_function_no_self() {
    // Associated functions (no self) should work without self-type issues
    let source = include_str!("../fixtures/integration/impl_associated_function_no_self.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Associated function without self should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_multiple_methods_all_use_self() {
    // Multiple methods in the same impl block should each get self bound correctly
    let source = include_str!("../fixtures/integration/impl_multiple_methods_all_use_self.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Multiple impl methods using self should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_method_bogus_param_type_rejected() {
    // A non-self parameter with a bogus type, when used in the body,
    // should produce a type mismatch (garbage != int).
    let source = include_str!("../fixtures/integration/impl_method_bogus_param_type_rejected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Impl method using bogus param type `garbage` in arithmetic should error"
    );
}

#[test]
fn impl_method_wrong_body_type_with_self_and_params() {
    // Body is int (self.value + amount), declared return is bool.
    // With self correctly typed, the mismatch must be detected.
    let source = include_str!(
        "../fixtures/integration/impl_method_wrong_body_type_with_self_and_params.ori"
    );
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Impl method body returning int but declared -> bool should error"
    );
}

#[test]
fn impl_self_method_on_enum() {
    // self should also work correctly on enum types
    let source = include_str!("../fixtures/integration/impl_self_method_on_enum.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Impl method with self on enum should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_self_method_on_single_field_struct() {
    // self should work on single-field struct types
    let source =
        include_str!("../fixtures/integration/impl_self_method_on_single_field_struct.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Impl method with self on single-field struct should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_self_passed_to_function_expecting_type() {
    // self should have the impl type, so passing it to a function that
    // expects that type should work
    let source =
        include_str!("../fixtures/integration/impl_self_passed_to_function_expecting_type.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Passing self to function expecting impl type should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn impl_self_passed_to_function_expecting_wrong_type() {
    // self is Point, but passed where str is expected — should error
    let source = include_str!(
        "../fixtures/integration/impl_self_passed_to_function_expecting_wrong_type.ori"
    );
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Passing self (Point) where str expected should error"
    );
}

// Never Type in Struct Fields (E2019)

#[test]
fn never_struct_field_rejected() {
    let source = include_str!("../fixtures/integration/never_struct_field_rejected.ori");
    let result = check_source(source);
    assert!(result.has_errors(), "Never struct field should be an error");
    assert!(
        result
            .error_kinds()
            .iter()
            .any(|k| matches!(k, TypeErrorKind::UninhabitedStructField { .. })),
        "Expected UninhabitedStructField error, got: {:?}",
        result.error_kinds()
    );
}

#[test]
fn never_in_sum_variant_allowed() {
    let source = include_str!("../fixtures/integration/never_in_sum_variant_allowed.ori");
    let result = check_source(source);
    assert!(
        !result
            .error_kinds()
            .iter()
            .any(|k| matches!(k, TypeErrorKind::UninhabitedStructField { .. })),
        "Never in sum variant should NOT produce UninhabitedStructField error"
    );
}
