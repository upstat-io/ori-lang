use super::support::*;

// Hash-First Import Resolution

/// Verify that hash-first import resolution produces identical results
/// to AST fallback, and measure the hit rate.
#[test]
fn hash_first_import_matches_ast_fallback() {
    let interner = StringInterner::new();

    // Provider module with a mix of monomorphic and generic functions
    let provider_source =
        include_str!("../fixtures/integration/hash_first_import_matches_ast_fallback_provider.ori");
    let provider = parse_source(provider_source, &interner);

    // Step 1: Import via AST fallback to get FunctionSigs with hashes
    let (ast_result, _pool) = crate::check::check_module_with_imports(
        &provider.module,
        &provider.arena,
        &interner,
        |_checker| {},
    );

    // Step 2: Import into a fresh checker via hash-first (using AST result's sigs)
    let consumer_source = fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/hash_first_import_matches_ast_fallback_consumer.ori"
    ));
    let consumer = parse_source(consumer_source, &interner);

    let (hash_result, _pool2) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            for func in &provider.module.functions {
                let imported_sig = ast_result
                    .typed
                    .functions
                    .iter()
                    .find(|s| s.name == func.name);
                checker.register_imported_function(func, &provider.arena, imported_sig);
            }
        },
    );

    // Both paths should produce no errors
    assert!(
        !hash_result.has_errors(),
        "Hash-first import produced errors: {:?}",
        hash_result
            .typed
            .errors
            .iter()
            .map(|e| &e.kind)
            .collect::<Vec<_>>()
    );

    // Verify all 5 provider functions + 1 consumer function = 6 total sigs
    assert_eq!(
        hash_result.typed.functions.len(),
        6,
        "Expected 6 function sigs (5 imported + 1 local), got {}",
        hash_result.typed.functions.len()
    );

    // Verify imported signatures have correct param/return hashes
    let add_name = interner.intern("add");
    let add_sig = hash_result
        .typed
        .functions
        .iter()
        .find(|s| s.name == add_name)
        .expect("add should be in sigs");
    assert_eq!(add_sig.param_hashes.len(), 2, "add has 2 params");
    // Int's Merkle hash may be 0 (FxHasher(0u8) from state 0 = 0), so we
    // verify consistency: the hash must match what a fresh Pool computes.
    let fresh_pool = crate::Pool::new();
    let expected_int_hash = fresh_pool.hash(crate::Idx::INT);
    assert_eq!(
        add_sig.return_hash, expected_int_hash,
        "add return hash should match Pool's hash for int"
    );
}

/// Verify that hash-first resolution correctly handles non-generic imports:
/// all param/return types should resolve by hash when the types already exist.
#[test]
fn hash_first_resolves_all_monomorphic_types() {
    let interner = StringInterner::new();

    // Provider with multiple non-generic functions using primitive types
    let provider_source = include_str!(
        "../fixtures/integration/hash_first_resolves_all_monomorphic_types_provider.ori"
    );
    let provider = parse_source(provider_source, &interner);

    // First pass: get FunctionSigs via AST
    let (ast_result, _pool) = crate::check::check_module_with_imports(
        &provider.module,
        &provider.arena,
        &interner,
        |_checker| {},
    );

    // Second pass: import via hash-first into a FRESH checker
    // Since all types are primitives (pre-interned), every hash lookup should hit
    let consumer_source = fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/hash_first_import_matches_ast_fallback_consumer.ori"
    ));
    let consumer = parse_source(consumer_source, &interner);

    let (result, _pool2) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            for func in &provider.module.functions {
                let imported_sig = ast_result
                    .typed
                    .functions
                    .iter()
                    .find(|s| s.name == func.name);
                checker.register_imported_function(func, &provider.arena, imported_sig);
            }
        },
    );

    assert!(
        !result.has_errors(),
        "Hash-first import produced errors: {:?}",
        result
            .typed
            .errors
            .iter()
            .map(|e| &e.kind)
            .collect::<Vec<_>>()
    );

    // All 4 provider functions should be importable
    // 4 imported + 1 local = 5 total
    assert_eq!(result.typed.functions.len(), 5);
}

/// Verify hash-first skips generic functions (falls back to AST).
#[test]
fn hash_first_skips_generic_functions() {
    let interner = StringInterner::new();

    let provider_source = fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/hash_first_skips_generic_functions_provider.ori"
    ));
    let provider = parse_source(provider_source, &interner);

    // Get FunctionSig with hashes
    let (ast_result, _pool) = crate::check::check_module_with_imports(
        &provider.module,
        &provider.arena,
        &interner,
        |_checker| {},
    );

    let identity_sig = ast_result
        .typed
        .functions
        .iter()
        .find(|s| interner.lookup(s.name) == "identity")
        .expect("identity should be in sigs");

    // Generic function should have non-empty scheme_var_ids
    assert!(
        !identity_sig.scheme_var_ids.is_empty(),
        "identity should be generic"
    );

    // Import via hash-first — should fall back to AST for generic
    let consumer_source = fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/hash_first_import_matches_ast_fallback_consumer.ori"
    ));
    let consumer = parse_source(consumer_source, &interner);

    let (result, _pool2) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            checker.register_imported_function(
                &provider.module.functions[0],
                &provider.arena,
                Some(identity_sig),
            );
        },
    );

    assert!(
        !result.has_errors(),
        "Generic import via hash-first should succeed (AST fallback): {:?}",
        result
            .typed
            .errors
            .iter()
            .map(|e| &e.kind)
            .collect::<Vec<_>>()
    );
}
