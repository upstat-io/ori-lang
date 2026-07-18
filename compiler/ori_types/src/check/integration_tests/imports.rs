use super::support::*;

// Cross-Module Imports

/// Result of checking a module with imports from another module.
struct ImportCheckResult {
    result: TypeCheckResult,
}

impl ImportCheckResult {
    fn has_errors(&self) -> bool {
        self.result.has_errors()
    }

    fn error_kinds(&self) -> Vec<&TypeErrorKind> {
        self.result.typed.errors.iter().map(|e| &e.kind).collect()
    }

    fn function_count(&self) -> usize {
        self.result.typed.functions.len()
    }
}

/// Check a module with imports registered from another parsed module.
fn check_with_imports(
    consumer_source: &str,
    provider_source: &str,
    interner: &StringInterner,
) -> ImportCheckResult {
    let provider = parse_source(provider_source, interner);
    let consumer = parse_source(consumer_source, interner);

    let (result, _pool) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        interner,
        |checker| {
            for func in &provider.module.functions {
                checker.register_imported_function(func, &provider.arena, None);
            }
        },
    );

    ImportCheckResult { result }
}

#[test]
fn import_simple_function() {
    // Module A exports `add(a: int, b: int) -> int`
    // Module B calls it with positional args (positional call is fully
    // type-checked; named call inference is not yet implemented)
    let interner = StringInterner::new();

    let result = check_with_imports(
        fixture_without_trailing_newline(include_str!(
            "../fixtures/integration/import_simple_function_consumer.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "../fixtures/integration/multiple_typed_params.ori"
        )),
        &interner,
    );

    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.error_kinds()
    );
    assert_eq!(result.function_count(), 2); // add (imported sig) + caller
}

#[test]
fn import_without_registration_fails() {
    // Module B lacks an import for `missing_fn`, so the call is unknown.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/import_without_registration_fails.ori"
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
fn import_function_with_different_types() {
    // Import `len(s: str) -> int`, call with correct types (positional)
    let interner = StringInterner::new();

    let result = check_with_imports(
        fixture_without_trailing_newline(include_str!(
            "../fixtures/integration/import_function_with_different_types_consumer.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "../fixtures/integration/import_function_with_different_types_provider.ori"
        )),
        &interner,
    );

    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.error_kinds()
    );
}

#[test]
fn import_return_type_mismatch_detected() {
    // Import `returns_str() -> str`, but consumer expects int → Mismatch
    // Uses the return type mismatch pattern since the checker fully
    // handles body-vs-signature checking but CallNamed is not yet implemented.
    let interner = StringInterner::new();

    let result = check_with_imports(
        fixture_without_trailing_newline(include_str!(
            "../fixtures/integration/import_return_type_mismatch_detected_consumer.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "../fixtures/integration/import_return_type_mismatch_detected_provider.ori"
        )),
        &interner,
    );

    assert!(result.has_errors());
    let has_mismatch = result
        .error_kinds()
        .iter()
        .any(|k| matches!(k, TypeErrorKind::Mismatch { .. }));
    assert!(
        has_mismatch,
        "Expected Mismatch error, got: {:?}",
        result.error_kinds()
    );
}

#[test]
fn import_does_not_shadow_local() {
    // Local `foo() -> int` should shadow imported `foo() -> str`
    let interner = StringInterner::new();

    let provider_source = fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/import_does_not_shadow_local_provider.ori"
    ));
    let consumer_source =
        include_str!("../fixtures/integration/import_does_not_shadow_local_consumer.ori");

    let provider = parse_source(provider_source, &interner);
    let consumer = parse_source(consumer_source, &interner);

    let (result, _pool) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            for func in &provider.module.functions {
                checker.register_imported_function(func, &provider.arena, None);
            }
        },
    );

    assert!(
        !result.has_errors(),
        "Expected no errors (local foo shadows import), got: {:?}",
        result
            .typed
            .errors
            .iter()
            .map(|e| &e.kind)
            .collect::<Vec<_>>()
    );

    // caller returns int (from local foo), not str
    let caller_name = interner.intern("caller");
    let caller_func = consumer
        .module
        .functions
        .iter()
        .find(|f| f.name == caller_name)
        .unwrap();
    let caller_body_ty = result
        .typed
        .expr_type(caller_func.body.raw() as usize)
        .unwrap();
    assert_eq!(caller_body_ty, Idx::INT);
}

#[test]
fn import_multiple_functions() {
    // Import two functions from the same module, call both in a chain (positional)
    let interner = StringInterner::new();

    let provider_source =
        include_str!("../fixtures/integration/import_multiple_functions_provider.ori");
    let consumer_source =
        include_str!("../fixtures/integration/import_multiple_functions_consumer.ori");

    let result = check_with_imports(consumer_source, provider_source, &interner);

    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.error_kinds()
    );
}

#[test]
fn import_module_alias_stores_signatures() {
    // Test that register_module_alias stores public function signatures
    let interner = StringInterner::new();
    let provider_source =
        include_str!("../fixtures/integration/import_module_alias_stores_signatures_provider.ori");
    let provider = parse_source(provider_source, &interner);
    let consumer = parse_source(
        fixture_without_trailing_newline(include_str!(
            "../fixtures/integration/import_module_alias_stores_signatures_consumer.ori"
        )),
        &interner,
    );

    let (result, _pool) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            let alias = interner.intern("math");
            checker.register_module_alias(alias, &provider.module, &provider.arena);

            // Verify: only the public function should be in the alias
            let aliases = checker.module_aliases();
            let math_sigs = aliases.get(&alias).unwrap();
            assert_eq!(math_sigs.len(), 1, "Only public functions in alias");
            assert!(math_sigs[0].is_public);
        },
    );

    assert!(
        !result.has_errors(),
        "Expected no errors, got: {:?}",
        result.errors()
    );
}

#[test]
fn module_alias_qualified_call_types_to_function_return() {
    // Spec: Clause 12. A module-qualified call has the aliased function's return type.
    let interner = StringInterner::new();
    let provider = parse_source(fixture_without_trailing_newline(include_str!("../fixtures/integration/module_alias_qualified_call_types_to_function_return_provider.ori")), &interner);
    let consumer = parse_source(fixture_without_trailing_newline(include_str!("../fixtures/integration/module_alias_qualified_call_types_to_function_return_consumer.ori")), &interner);

    let (result, _pool) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            let alias = interner.intern("math");
            checker.register_module_alias(alias, &provider.module, &provider.arena);
        },
    );

    assert!(
        !result.has_errors(),
        "qualified module-alias call must type-check; errors: {:?}",
        result
            .typed
            .errors
            .iter()
            .map(|e| &e.kind)
            .collect::<Vec<_>>()
    );

    let caller_name = interner.intern("caller");
    let caller_func = consumer
        .module
        .functions
        .iter()
        .find(|f| f.name == caller_name)
        .unwrap();
    let caller_body_ty = result
        .typed
        .expr_type(caller_func.body.raw() as usize)
        .unwrap();
    assert_eq!(
        caller_body_ty,
        Idx::INT,
        "math.add(...) must type to the aliased function's int return, not Idx::ERROR"
    );
}

#[test]
fn module_alias_unknown_qualified_method_does_not_resolve() {
    // Negative pin: a qualified call to a function NOT exported by the aliased
    // module must NOT spuriously resolve to a concrete type via the
    // module-alias path. `math.nonexistent(...)` has no matching signature, so
    // the resolver returns None and the call falls through to ordinary method
    // dispatch (which finds nothing on the namespace placeholder).
    let interner = StringInterner::new();
    let provider = parse_source(fixture_without_trailing_newline(include_str!("../fixtures/integration/module_alias_qualified_call_types_to_function_return_provider.ori")), &interner);
    let consumer = parse_source(fixture_without_trailing_newline(include_str!("../fixtures/integration/module_alias_unknown_qualified_method_does_not_resolve_consumer.ori")), &interner);

    let (result, _pool) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            let alias = interner.intern("math");
            checker.register_module_alias(alias, &provider.module, &provider.arena);
        },
    );

    let caller_name = interner.intern("caller");
    let caller_func = consumer
        .module
        .functions
        .iter()
        .find(|f| f.name == caller_name)
        .unwrap();
    let caller_body_ty = result
        .typed
        .expr_type(caller_func.body.raw() as usize)
        .unwrap();
    assert_ne!(
        caller_body_ty,
        Idx::INT,
        "math.nonexistent(...) must NOT resolve to int via the module-alias path"
    );
}
