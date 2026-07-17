#![expect(
    clippy::expect_used,
    reason = "integration fixtures abort when parsing or required registry lookup fails"
)]
//! Integration tests for the module checker.
//!
//! These tests feed real Ori source code through the full pipeline:
//! lexer → parser → type checker, verifying the end-to-end behavior.
//!
//! # Test Categories
//!
//! - **Literals**: Basic literal expressions in function bodies
//! - **Parameters**: Typed function parameters
//! - **Multi-function**: Forward references, mutual recursion
//! - **Tests**: `@test` declarations
//! - **Type errors**: Mismatches, unknown identifiers
//! - **Let bindings**: Local variable bindings
//! - **Control flow**: If/then/else expressions
//! - **Collections**: List literals
//! - **Operators**: Arithmetic, comparison, boolean
//! - **Empty module**: No declarations

#![expect(
    clippy::unwrap_used,
    reason = "integration fixtures treat unexpected type-check errors as assertion failures"
)]

use ori_ir::StringInterner;

use crate::check::check_module_with_pool;
use crate::{Idx, Pool, Tag, TypeCheckResult, TypeErrorKind};

fn fixture_without_trailing_newline(source: &'static str) -> &'static str {
    source
        .strip_suffix('\n')
        .expect("committed Ori fixtures end with a newline")
}

// Test Infrastructure

/// Result of checking a source string through the full pipeline.
struct CheckResult {
    result: TypeCheckResult,
    pool: Pool,
    interner: StringInterner,
    parsed: ori_parse::ParseOutput,
}

impl CheckResult {
    /// Whether any type errors were reported.
    fn has_errors(&self) -> bool {
        self.result.has_errors()
    }

    /// Number of type errors.
    fn error_count(&self) -> usize {
        self.result.typed.errors.len()
    }

    /// Number of functions in the typed module.
    fn function_count(&self) -> usize {
        self.result.typed.functions.len()
    }

    /// Get all error kinds for assertion.
    fn error_kinds(&self) -> Vec<&TypeErrorKind> {
        self.result.typed.errors.iter().map(|e| &e.kind).collect()
    }

    /// Look up the body expression type of the first function.
    ///
    /// Returns the type of the function's body expression (its return value).
    fn first_function_body_type(&self) -> Option<Idx> {
        let func = self.parsed.module.functions.first()?;
        let body_index = func.body.raw() as usize;
        self.result.typed.expr_type(body_index)
    }

    /// Look up the body expression type of a function by name.
    fn function_body_type(&self, name: &str) -> Option<Idx> {
        let name_id = self.interner.intern(name);
        let func = self
            .parsed
            .module
            .functions
            .iter()
            .find(|f| f.name == name_id)?;
        let body_index = func.body.raw() as usize;
        self.result.typed.expr_type(body_index)
    }

    /// Get the tag (type kind) of a resolved type.
    fn tag(&self, idx: Idx) -> Tag {
        self.pool.tag(idx)
    }

    /// Find mono instances for a given function name.
    fn mono_instances_for(&self, name: &str) -> Vec<&crate::MonoInstance> {
        let name_id = self.interner.intern(name);
        self.result
            .typed
            .mono_instances
            .iter()
            .filter(|m| m.fn_name == name_id)
            .collect()
    }

    /// All mono instances recorded for the module (name-agnostic).
    ///
    /// Returns every recorded mono instance without filtering by function name.
    /// Builtin-only tests use the complete set when name resolution does not
    /// expose a stable function name.
    fn mono_instances_all(&self) -> &[crate::MonoInstance] {
        &self.result.typed.mono_instances
    }

    /// Find the first `Tag::Applied` pool entry whose name and resolved args
    /// match `name` + `args`. Used by the generic-composite-monomorphization
    /// pins to locate the `Applied(Generic, [concrete])` handle and inspect its
    /// `Pool.resolutions` materialization.
    fn find_applied(&self, name: &str, args: &[Idx]) -> Option<Idx> {
        let name_id = self.interner.intern(name);
        let len =
            u32::try_from(self.pool.len()).expect("type pool length must fit the Idx u32 domain");
        (Idx::FIRST_DYNAMIC..len).map(Idx::from_raw).find(|&idx| {
            self.pool.tag(idx) == Tag::Applied
                && self.pool.applied_name(idx) == name_id
                && self.pool.applied_args(idx).as_slice() == args
        })
    }
}

/// Parse and type-check an Ori source string.
fn check_source(source: &str) -> CheckResult {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    // Ensure no parse errors before type checking
    assert!(
        parsed.errors.is_empty(),
        "Parse errors in test source: {:?}",
        parsed.errors
    );

    let (result, pool) = check_module_with_pool(&parsed.module, &parsed.arena, &interner);

    CheckResult {
        result,
        pool,
        interner,
        parsed,
    }
}

/// Parse and type-check, allowing parse errors (for testing that we handle them).
fn check_source_allow_parse_errors(source: &str) -> CheckResult {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);
    let (result, pool) = check_module_with_pool(&parsed.module, &parsed.arena, &interner);

    CheckResult {
        result,
        pool,
        interner,
        parsed,
    }
}

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
        "fixtures/integration/literal_int.ori"
    )));
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 1);

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn literal_float() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/literal_float.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::FLOAT);
}

#[test]
fn literal_bool() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/literal_bool.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn literal_string() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/literal_string.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn literal_unit() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/literal_unit.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::UNIT);
}

// Function Parameters

#[test]
fn single_typed_param() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/single_typed_param.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn multiple_typed_params() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/multiple_typed_params.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn param_type_used_in_body() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/param_type_used_in_body.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

// Multiple Functions

#[test]
fn two_functions() {
    let source = include_str!("fixtures/integration/two_functions.ori");
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
    let source = include_str!("fixtures/integration/function_calling_another.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

#[test]
fn forward_reference() {
    // bar defined before foo, but calls foo
    let source = include_str!("fixtures/integration/forward_reference.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

// Test Declarations

#[test]
fn test_declaration() {
    let source = include_str!("fixtures/integration/declaration.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    // Functions + tests both counted as signatures
    assert_eq!(result.function_count(), 2);
}

#[test]
fn test_with_function_call() {
    // Test body that uses the target function via block expression
    let source = include_str!("fixtures/integration/with_function_call.ori");
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
        "fixtures/integration/return_type_mismatch.ori"
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
        "fixtures/integration/unknown_identifier_in_body.ori"
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
    let source = include_str!("fixtures/integration/unknown_identifier_suggests_similar_names.ori");
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
        "fixtures/integration/unknown_identifier_no_suggestion_for_unrelated_names.ori"
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
    let source = include_str!("fixtures/integration/call_with_named_arg.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

// Let Bindings

#[test]
fn simple_let_binding() {
    let source = include_str!("fixtures/integration/simple_let_binding.ori");
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
    let source = include_str!("fixtures/integration/let_in_block_body.ori");
    let result = check_source(source);
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

// Control Flow

#[test]
fn if_then_else_int() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/if_then_else_int.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn if_then_else_string() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/if_then_else_string.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn if_condition_must_be_bool() {
    // Using an int as condition should produce an error
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/if_condition_must_be_bool.ori"
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

// Collections

#[test]
fn list_literal() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/list_literal.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(result.tag(body_ty), Tag::List);
}

#[test]
fn empty_list() {
    // Empty list with type annotation on function
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/empty_list.ori"
    )));
    // The empty list may or may not unify with [int] depending on inference
    // At minimum, it shouldn't panic
    let _ = result.has_errors();
}

// Operators

#[test]
fn arithmetic_operators() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/arithmetic_operators.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn comparison_operators() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/comparison_operators.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn boolean_operators() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/boolean_operators.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn equality_check() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/equality_check.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn string_concatenation() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/string_concatenation.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn negation() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/negation.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn boolean_not() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/boolean_not.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

// Tuple Expressions

#[test]
fn tuple_literal() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/tuple_literal.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(result.tag(body_ty), Tag::Tuple);
}

// Multiple Error Accumulation

#[test]
fn multiple_errors_accumulated() {
    // Two functions with errors - should accumulate both
    let source = include_str!("fixtures/integration/multiple_errors_accumulated.ori");
    let result = check_source(source);
    assert!(result.has_errors());
    // Should have at least 2 errors (one per function)
    assert!(
        result.error_count() >= 2,
        "Expected at least 2 errors, got {}",
        result.error_count()
    );
}

// Cross-Module Imports

/// Parse source into a `ParseOutput` using a shared interner.
///
/// This is the building block for cross-module import tests: each module
/// is parsed independently (with its own arena) but shares a string interner
/// so that `Name` handles are consistent across modules.
fn parse_source(source: &str, interner: &StringInterner) -> ori_parse::ParseOutput {
    let tokens = ori_lexer::lex(source, interner);
    let parsed = ori_parse::parse(&tokens, interner);
    assert!(
        parsed.errors.is_empty(),
        "Parse errors in test source: {:?}",
        parsed.errors
    );
    parsed
}

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
            "fixtures/integration/import_simple_function_consumer.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/integration/multiple_typed_params.ori"
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
        "fixtures/integration/import_without_registration_fails.ori"
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
            "fixtures/integration/import_function_with_different_types_consumer.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/integration/import_function_with_different_types_provider.ori"
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
            "fixtures/integration/import_return_type_mismatch_detected_consumer.ori"
        )),
        fixture_without_trailing_newline(include_str!(
            "fixtures/integration/import_return_type_mismatch_detected_provider.ori"
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
        "fixtures/integration/import_does_not_shadow_local_provider.ori"
    ));
    let consumer_source =
        include_str!("fixtures/integration/import_does_not_shadow_local_consumer.ori");

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
        include_str!("fixtures/integration/import_multiple_functions_provider.ori");
    let consumer_source =
        include_str!("fixtures/integration/import_multiple_functions_consumer.ori");

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
        include_str!("fixtures/integration/import_module_alias_stores_signatures_provider.ori");
    let provider = parse_source(provider_source, &interner);
    let consumer = parse_source(
        fixture_without_trailing_newline(include_str!(
            "fixtures/integration/import_module_alias_stores_signatures_consumer.ori"
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
    let provider = parse_source(fixture_without_trailing_newline(include_str!("fixtures/integration/module_alias_qualified_call_types_to_function_return_provider.ori")), &interner);
    let consumer = parse_source(fixture_without_trailing_newline(include_str!("fixtures/integration/module_alias_qualified_call_types_to_function_return_consumer.ori")), &interner);

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
    let provider = parse_source(fixture_without_trailing_newline(include_str!("fixtures/integration/module_alias_qualified_call_types_to_function_return_provider.ori")), &interner);
    let consumer = parse_source(fixture_without_trailing_newline(include_str!("fixtures/integration/module_alias_unknown_qualified_method_does_not_resolve_consumer.ori")), &interner);

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
        "fixtures/integration/function_returning_void.ori"
    )));
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::UNIT);
}

#[test]
fn many_functions() {
    let source = include_str!("fixtures/integration/many_functions.ori");
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 5);
}

// Type Definition Exports

#[test]
fn struct_type_exported() {
    let source = include_str!("fixtures/integration/struct_type_exported.ori");
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
    let source = include_str!("fixtures/integration/enum_type_exported.ori");
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
    let source = include_str!("fixtures/integration/bogus_return_type_is_rejected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined return type `garbage`, got none"
    );
}

#[test]
fn bogus_return_type_on_method_is_rejected() {
    // Same bug but on a method with `self` — this is the user's exact repro
    let source = include_str!("fixtures/integration/bogus_return_type_on_method_is_rejected.ori");
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
        include_str!("fixtures/integration/bogus_return_type_in_impl_block_is_rejected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined return type `nt` in impl block, got none"
    );
}

#[test]
fn bogus_param_type_is_rejected() {
    // Also check parameter types — `garbage` as a param type should error
    let source = include_str!("fixtures/integration/bogus_param_type_is_rejected.ori");
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
    let source = include_str!("fixtures/integration/bogus_return_type_on_method_is_rejected.ori");
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
fn valid_return_type_still_works() {
    // A valid return annotation produces no type errors.
    let source = include_str!("fixtures/integration/valid_return_type_still_works.ori");
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
    let source = include_str!("fixtures/integration/impl_self_field_access_type_checks.ori");
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
    let source = include_str!("fixtures/integration/impl_self_with_additional_params.ori");
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
    let source = include_str!("fixtures/integration/impl_self_return_type_mismatch_detected.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Impl method returning int but declared -> str should error"
    );
}

#[test]
fn impl_self_returning_self_type() {
    // Self as return type should resolve to the impl type
    let source = include_str!("fixtures/integration/impl_self_returning_self_type.ori");
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
    let source = include_str!("fixtures/integration/impl_associated_function_no_self.ori");
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
    let source = include_str!("fixtures/integration/impl_multiple_methods_all_use_self.ori");
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
    let source = include_str!("fixtures/integration/impl_method_bogus_param_type_rejected.ori");
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
    let source =
        include_str!("fixtures/integration/impl_method_wrong_body_type_with_self_and_params.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Impl method body returning int but declared -> bool should error"
    );
}

#[test]
fn impl_self_method_on_enum() {
    // self should also work correctly on enum types
    let source = include_str!("fixtures/integration/impl_self_method_on_enum.ori");
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
    let source = include_str!("fixtures/integration/impl_self_method_on_single_field_struct.ori");
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
        include_str!("fixtures/integration/impl_self_passed_to_function_expecting_type.ori");
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
    let source =
        include_str!("fixtures/integration/impl_self_passed_to_function_expecting_wrong_type.ori");
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Passing self (Point) where str expected should error"
    );
}

// Never Type in Struct Fields (E2019)

#[test]
fn never_struct_field_rejected() {
    let source = include_str!("fixtures/integration/never_struct_field_rejected.ori");
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
    let source = include_str!("fixtures/integration/never_in_sum_variant_allowed.ori");
    let result = check_source(source);
    assert!(
        !result
            .error_kinds()
            .iter()
            .any(|k| matches!(k, TypeErrorKind::UninhabitedStructField { .. })),
        "Never in sum variant should NOT produce UninhabitedStructField error"
    );
}

// Collect trait — bidirectional type inference

fn assert_collect_body_tag(source: &str, function: &str, expected: Tag) {
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "collect() should not error: {:?}",
        result.error_kinds()
    );
    let ty = result
        .function_body_type(function)
        .unwrap_or_else(|| panic!("missing body type for {function}"));
    assert_eq!(result.tag(ty), expected, "unexpected collect body type");
}

#[test]
fn collect_to_set_via_return_type() {
    // Return type `Set<int>` should guide `collect()` to produce Set
    assert_collect_body_tag(
        include_str!("fixtures/integration/collect_to_set_via_return_type.ori"),
        "to_set",
        Tag::Set,
    );
}

#[test]
fn collect_to_list_by_default() {
    // No Set annotation — collect() should default to list
    assert_collect_body_tag(
        include_str!("fixtures/integration/collect_to_list_by_default.ori"),
        "to_list",
        Tag::List,
    );
}

#[test]
fn collect_to_set_via_let_binding() {
    // Let binding with Set<int> annotation should guide collect()
    let source = include_str!("fixtures/integration/collect_to_set_via_let_binding.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "collect() via let binding with Set<int> should not error: {:?}",
        result.error_kinds()
    );
}

#[test]
fn collect_chained_adapters_to_set() {
    // Chained adapters (filter) before collect should preserve Set inference
    assert_collect_body_tag(
        include_str!("fixtures/integration/collect_chained_adapters_to_set.ori"),
        "filtered",
        Tag::Set,
    );
}

// Monomorphization Instance Recording

#[test]
fn generic_identity_records_mono_instance() {
    let source = include_str!("fixtures/integration/generic_identity_records_mono_instance.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("identity");
    assert!(
        !instances.is_empty(),
        "identity called with int should record a mono instance"
    );
    // The concrete arg should be int.
    assert_eq!(instances[0].concrete_param_types.len(), 1);
    assert_eq!(instances[0].concrete_param_types[0], Idx::INT);
    assert_eq!(instances[0].concrete_return_type, Idx::INT);
}

#[test]
fn generic_two_param_records_mono_instance() {
    let source = include_str!("fixtures/integration/generic_two_param_records_mono_instance.ori"); // Needs r#"..."# because of the " in "hello"
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("pair");
    assert!(
        !instances.is_empty(),
        "pair called with (int, str) should record a mono instance"
    );
    assert_eq!(instances[0].concrete_param_types.len(), 2);
    assert_eq!(instances[0].concrete_param_types[0], Idx::INT);
    assert_eq!(instances[0].concrete_param_types[1], Idx::STR);
}

#[test]
fn non_generic_call_records_nothing() {
    let source = include_str!("fixtures/integration/non_generic_call_records_nothing.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("add");
    assert!(
        instances.is_empty(),
        "non-generic function should not produce mono instances"
    );
}

#[test]
fn same_generic_call_twice_deduplicates() {
    let source = include_str!("fixtures/integration/same_generic_call_twice_deduplicates.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("identity");
    // Both calls use int, so dedup should give exactly one instance.
    assert_eq!(
        instances
            .iter()
            .filter(|m| m.concrete_param_types[0] == Idx::INT)
            .count(),
        1,
        "same generic args should dedup to one instance"
    );
}

#[test]
fn different_type_args_produce_separate_instances() {
    let source =
        include_str!("fixtures/integration/different_type_args_produce_separate_instances.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("identity");
    // Should have exactly 2 distinct instances: one for int, one for str.
    assert_eq!(
        instances.len(),
        2,
        "identity<int> and identity<str> should produce 2 instances, got: {instances:?}"
    );
    let types: Vec<Idx> = instances
        .iter()
        .map(|m| m.concrete_param_types[0])
        .collect();
    assert!(types.contains(&Idx::INT), "should have int instance");
    assert!(types.contains(&Idx::STR), "should have str instance");
}

// Method Monomorphization — Inherent Methods on Generic Receivers
//
// An inherent method on a generic impl (`impl<T> Box<T> { @unwrap (self) -> T }`)
// called on a concretely-instantiated receiver (`Box<int>`) records a
// receiver-bearing MonoInstance so codegen can monomorphize it. The instance
// carries `receiver_type = Some(Box<int>)` and `impl_args = [int]`.

#[test]
fn inherent_method_on_generic_receiver_records_method_instance() {
    let source = include_str!(
        "fixtures/integration/inherent_method_on_generic_receiver_records_method_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        1,
        "Box<int>.unwrap() should record exactly one method instance, got: {instances:?}"
    );
    let inst = instances[0];
    assert!(
        inst.receiver_type.is_some(),
        "method instance must carry receiver_type, got None"
    );
    assert_eq!(
        inst.impl_args,
        vec![crate::GenericArg::Type(Idx::INT)],
        "impl_args should be [int] for Box<int>.unwrap()"
    );
    assert!(
        inst.method_args.is_empty(),
        "unwrap has no method-level generics, got: {:?}",
        inst.method_args
    );
    assert_eq!(
        inst.concrete_return_type,
        Idx::INT,
        "unwrap on Box<int> returns int"
    );
}

#[test]
fn same_method_on_same_receiver_deduplicates() {
    let source = include_str!("fixtures/integration/same_method_on_same_receiver_deduplicates.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        1,
        "two Box<int>.unwrap() calls should dedup to one instance, got: {instances:?}"
    );
}

#[test]
fn method_on_distinct_receivers_produces_separate_instances() {
    let source = include_str!(
        "fixtures/integration/method_on_distinct_receivers_produces_separate_instances.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        2,
        "Box<int>.unwrap() and Box<str>.unwrap() should produce 2 instances, got: {instances:?}"
    );
    let impl_args: Vec<&Vec<crate::GenericArg>> = instances.iter().map(|m| &m.impl_args).collect();
    assert!(
        impl_args.contains(&&vec![crate::GenericArg::Type(Idx::INT)]),
        "expected an [int] impl_args instance, got: {impl_args:?}"
    );
    assert!(
        impl_args.contains(&&vec![crate::GenericArg::Type(Idx::STR)]),
        "expected a [str] impl_args instance, got: {impl_args:?}"
    );
}

/// Regression: a generic-receiver inherent method on a nested-generic receiver
/// (`Box<[int]>`, whose type argument is itself a generic) records a `MonoInstance`
/// whose `impl_args` carries the full nested type — distinct from scalar `Box<int>`.
/// A recorder that collapsed the nested `[int]` to the generic shell would dedup
/// the two receivers into one instance, re-surfacing the missing-mono condition.
#[test]
fn method_on_nested_generic_receiver_records_distinct_instance() {
    let source = include_str!(
        "fixtures/integration/method_on_nested_generic_receiver_records_distinct_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("unwrap");
    assert_eq!(
        instances.len(),
        2,
        "Box<int>.unwrap() and Box<[int]>.unwrap() should produce 2 distinct instances, got: {instances:?}"
    );
    // Exactly one instance is the scalar `Box<int>` ([int] impl_args); the other
    // is the nested-generic `Box<[int]>`, whose impl_args is NOT [int]. Equal
    // impl_args would mean the recorder collapsed the nested generic to the shell.
    assert_ne!(
        instances[0].impl_args,
        instances[1].impl_args,
        "nested-generic Box<[int]> and scalar Box<int> must record distinct impl_args, got: {:?}",
        instances.iter().map(|m| &m.impl_args).collect::<Vec<_>>()
    );
    let scalar_count = instances
        .iter()
        .filter(|m| m.impl_args == vec![crate::GenericArg::Type(Idx::INT)])
        .count();
    assert_eq!(
        scalar_count, 1,
        "exactly one instance is the scalar Box<int> ([int] impl_args); the nested Box<[int]> carries the list type, got: {:?}",
        instances.iter().map(|m| &m.impl_args).collect::<Vec<_>>()
    );
}

#[test]
fn inherent_method_on_non_generic_receiver_records_nothing() {
    // The impl is NOT generic over the receiver's type params, so the method
    // call must emit no method MonoInstance (the additive scope guard leaves
    // non-generic inherent dispatch untouched).
    let source = include_str!(
        "fixtures/integration/inherent_method_on_non_generic_receiver_records_nothing.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "should not error: {:?}",
        result.error_kinds()
    );

    let instances = result.mono_instances_for("get");
    assert!(
        instances.is_empty(),
        "non-generic-receiver inherent method should record no instance, got: {instances:?}"
    );
}

// Deferred Monomorphization — Union-Find Root Extension
//
// Rank-weighted union-find can make a fresh
// instantiation var the root of a scheme var's equivalence class. Without
// `extend_var_subst_with_roots` the deferred-resolve path leaves the root
// var's `Tag::Var` leaf unsubstituted in the callee body, leaking through
// to ARC IR where the PC-2 seam fires. These tests verify that a
// multi-hop generic forwarding chain produces fully-concrete MonoInstances
// for every intermediate callee — signalling the root-extension fired on
// the deferred path.

/// Return `true` when every concrete parameter and return type is free of
/// unresolved variables after deferred resolution.
fn instance_signatures_fully_concrete(result: &CheckResult, fn_name: &str) -> bool {
    let instances = result.mono_instances_for(fn_name);
    if instances.is_empty() {
        return false;
    }
    for inst in instances {
        if result.pool.flags(inst.concrete_return_type).has_vars() {
            return false;
        }
        for &p in &inst.concrete_param_types {
            if result.pool.flags(p).has_vars() {
                return false;
            }
        }
    }
    true
}

/// Return `true` iff the `MonoInstance`s for `fn_name` all contain at least
/// one `body_type_map` entry whose value is `expected_concrete`. This is
/// the positive pin: without the root-extension, a `Tag::Var` ROOT leaf
/// in the callee's body would fall through `substitute_var` unchanged and
/// never produce a mapping to `expected_concrete` in the entry list.
fn instance_body_type_map_covers_concrete(
    result: &CheckResult,
    fn_name: &str,
    expected_concrete: Idx,
) -> bool {
    let instances = result.mono_instances_for(fn_name);
    if instances.is_empty() {
        return false;
    }
    instances.iter().all(|inst| {
        inst.body_type_map
            .iter()
            .any(|(_, v)| *v == expected_concrete)
    })
}

#[test]
fn deferred_mono_resolution_root_extension_applied_3_hop() {
    // 3-hop forwarder chain: @main → @double_wrap<int> → @wrap<int> → @id<int>.
    // The two middle hops are deferred monomorphization calls (generic
    // calling generic). With the root-extension fix, every MonoInstance
    // produced for the chain is fully concrete and every body_type_map
    // entry substitutes to a non-Var concrete type.
    let source = include_str!(
        "fixtures/integration/deferred_mono_resolution_root_extension_applied_3_hop.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "3-hop forwarder chain should type-check: {:?}",
        result.error_kinds()
    );

    // Each of the three generic functions should have at least one
    // MonoInstance recorded for T = int (direct for @double_wrap from
    // @main; deferred for @wrap and @id).
    for fn_name in ["id", "wrap", "double_wrap"] {
        let instances = result.mono_instances_for(fn_name);
        assert!(
            !instances.is_empty(),
            "{fn_name} should have a MonoInstance recorded for T = int"
        );

        assert!(
            instance_signatures_fully_concrete(&result, fn_name),
            "{fn_name} MonoInstance param/return types must be fully concrete \
             (no Tag::Var); a leaked Var signals the root-extension did NOT \
             fire on the deferred path. Instances: {instances:?}"
        );

        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::INT),
            "{fn_name}.body_type_map must contain an entry mapping to Idx::INT \
             — the root-extension's job is to route the callee body's Tag::Var \
             root-leaves through var_subst so they materialize in body_type_map"
        );
    }

    // Positive pin on the int instance: param + return are Idx::INT.
    let wrap_int_instances: Vec<_> = result
        .mono_instances_for("wrap")
        .into_iter()
        .filter(|m| m.concrete_param_types == vec![Idx::INT])
        .collect();
    assert_eq!(
        wrap_int_instances.len(),
        1,
        "wrap<int> should have exactly one MonoInstance, got {} — {wrap_int_instances:?}",
        wrap_int_instances.len()
    );
    assert_eq!(wrap_int_instances[0].concrete_return_type, Idx::INT);
}

#[test]
fn deferred_mono_resolution_root_extension_applied_4_hop() {
    // 4-hop chain: @main → @a → @b → @c → @d. The three middle callees
    // (@b, @c, @d) are deferred. Verifies the root-extension holds beyond
    // 3-hop — guards against off-by-one in transitive resolution.
    let source = include_str!(
        "fixtures/integration/deferred_mono_resolution_root_extension_applied_4_hop.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "4-hop forwarder chain should type-check: {:?}",
        result.error_kinds()
    );

    for fn_name in ["a", "b", "c", "d"] {
        let instances = result.mono_instances_for(fn_name);
        assert!(
            !instances.is_empty(),
            "{fn_name} should have a MonoInstance recorded for T = int"
        );

        assert!(
            instance_signatures_fully_concrete(&result, fn_name),
            "{fn_name} MonoInstance param/return types must be fully concrete"
        );

        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::INT),
            "{fn_name}.body_type_map must contain an Idx::INT target"
        );
    }

    // Positive pin: int propagates all the way down through the chain.
    for fn_name in ["a", "b", "c", "d"] {
        let int_instances: Vec<_> = result
            .mono_instances_for(fn_name)
            .into_iter()
            .filter(|m| m.concrete_param_types == vec![Idx::INT])
            .collect();
        assert_eq!(
            int_instances.len(),
            1,
            "{fn_name}<int> should have exactly one MonoInstance"
        );
        assert_eq!(int_instances[0].concrete_return_type, Idx::INT);
    }
}

#[test]
fn deferred_mono_resolution_multi_param_forwarding() {
    // Multi-param forwarder with REORDERED arguments:
    //   @f<A, B> (x: A, y: B) -> B = g(x: y, y: x)
    // At @g's instantiation A_g ← B_f and B_g ← A_f — the union-find
    // root walk must handle each scheme var independently. With the
    // root-extension, @g's MonoInstance comes out fully concrete at
    // every call site; without it, the reordered binding can leave
    // Tag::Var leaves depending on which scheme var roots the class.
    let source =
        include_str!("fixtures/integration/deferred_mono_resolution_multi_param_forwarding.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "multi-param forwarder chain should type-check: {:?}",
        result.error_kinds()
    );

    for fn_name in ["f", "g"] {
        let instances = result.mono_instances_for(fn_name);
        assert!(
            !instances.is_empty(),
            "{fn_name} should have a MonoInstance recorded"
        );

        assert!(
            instance_signatures_fully_concrete(&result, fn_name),
            "{fn_name} MonoInstance param/return types must be fully concrete"
        );
    }

    // Coverage pin: the body_type_map must route to both Idx::INT and
    // Idx::STR — the concrete types threaded through A and B.
    for fn_name in ["f", "g"] {
        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::INT),
            "{fn_name}.body_type_map must contain an Idx::INT target"
        );
        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::STR),
            "{fn_name}.body_type_map must contain an Idx::STR target"
        );
    }

    // Positive pin: @f<int, str> has param types (int, str) and return str.
    let f_instances = result.mono_instances_for("f");
    let f_int_str: Vec<_> = f_instances
        .iter()
        .filter(|m| m.concrete_param_types == vec![Idx::INT, Idx::STR])
        .collect();
    assert_eq!(
        f_int_str.len(),
        1,
        "f<int, str> should have exactly one MonoInstance, got {}",
        f_int_str.len()
    );
    assert_eq!(f_int_str[0].concrete_return_type, Idx::STR);

    // Positive pin: @g is called inside @f with reordered args — the
    // resulting @g instance has (B_f, A_f) = (str, int) as its concrete
    // param types and A_f = int as its return type.
    let g_instances = result.mono_instances_for("g");
    let g_str_int: Vec<_> = g_instances
        .iter()
        .filter(|m| m.concrete_param_types == vec![Idx::STR, Idx::INT])
        .collect();
    assert_eq!(
        g_str_int.len(),
        1,
        "g<str, int> (reordered from f) should have exactly one MonoInstance, got {}",
        g_str_int.len()
    );
    assert_eq!(g_str_int[0].concrete_return_type, Idx::INT);
}

// Hash-First Import Resolution

/// Verify that hash-first import resolution produces identical results
/// to AST fallback, and measure the hit rate.
#[test]
fn hash_first_import_matches_ast_fallback() {
    let interner = StringInterner::new();

    // Provider module with a mix of monomorphic and generic functions
    let provider_source =
        include_str!("fixtures/integration/hash_first_import_matches_ast_fallback_provider.ori");
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
        "fixtures/integration/hash_first_import_matches_ast_fallback_consumer.ori"
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
    let provider_source =
        include_str!("fixtures/integration/hash_first_resolves_all_monomorphic_types_provider.ori");
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
        "fixtures/integration/hash_first_import_matches_ast_fallback_consumer.ori"
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
        "fixtures/integration/hash_first_skips_generic_functions_provider.ori"
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
        "fixtures/integration/hash_first_import_matches_ast_fallback_consumer.ori"
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

// E2048 EDROP_PARTIAL_MOVE — match-destructure whole-value-consumption
// invariant. A PARTIAL by-value destructure of a Drop type (binding a proper
// subset of owned fields, leaving the residual live) is a double-free / leak
// hazard; the bind-all-fields whole-value case is the sound consumption form.
// Spec: drop-trait-proposal.md §Execution Timing.
//
// `check_source` does not load the prelude, so each source declares a local
// `trait Drop` to register it in the trait registry (the validator silently
// no-ops when `Drop` is unregistered — the pre-deployment shape).

impl CheckResult {
    /// Count `DropPartialMove` (E2048) errors.
    fn drop_partial_move_count(&self) -> usize {
        self.error_kinds()
            .iter()
            .filter(|k| matches!(k, TypeErrorKind::DropPartialMove { .. }))
            .count()
    }
}

fn assert_drop_partial_move_count(source: &'static str, expected: usize, context: &str) {
    let result = check_source(fixture_without_trailing_newline(source));
    assert_eq!(
        result.drop_partial_move_count(),
        expected,
        "{context}; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_partial_struct_destructure_rejected_e2048() {
    // Negative pin: `Pair { a, .. }` binds 1 of 2 owned fields by value —
    // proper subset → E2048.
    assert_drop_partial_move_count(
        include_str!(
            "fixtures/integration/drop_match_partial_struct_destructure_rejected_e2048.ori"
        ),
        1,
        "partial struct destructure of a Drop type MUST fire exactly one E2048",
    );
}

#[test]
fn drop_match_whole_value_struct_destructure_accepted() {
    // Positive pin: `Pair { a, b }` binds EVERY owned field — whole-value
    // consumption → no E2048.
    assert_drop_partial_move_count(
        include_str!(
            "fixtures/integration/drop_match_whole_value_struct_destructure_accepted.ori"
        ),
        0,
        "whole-value struct destructure of a Drop type MUST NOT fire E2048",
    );
}

#[test]
fn drop_match_partial_enum_variant_destructure_rejected_e2048() {
    // Negative pin: `Pair(x)` binds 1 of 2 payload fields → E2048.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/drop_match_partial_enum_variant_destructure_rejected_e2048.ori"
    )));
    assert!(
        result.drop_partial_move_count() >= 1,
        "partial enum-variant destructure of a Drop type MUST fire E2048; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_whole_payload_enum_variant_destructure_accepted() {
    // Positive pin: `Pair(x, y)` binds every payload field of the matched
    // variant → whole-value consumption → no E2048.
    assert_drop_partial_move_count(
        include_str!(
            "fixtures/integration/drop_match_whole_payload_enum_variant_destructure_accepted.ori"
        ),
        0,
        "whole-payload enum-variant destructure MUST NOT fire E2048",
    );
}

#[test]
fn drop_match_partial_destructure_on_non_drop_type_accepted() {
    // Negative-space pin: the E2048 axis is Drop-only. A partial destructure of
    // a NON-Drop type must NOT fire E2048 (it is governed by E2043's
    // conditional-move axis, not E2048's unconditional-Drop axis).
    assert_drop_partial_move_count(
        include_str!(
            "fixtures/integration/drop_match_partial_destructure_on_non_drop_type_accepted.ori"
        ),
        0,
        "partial destructure of a non-Drop type MUST NOT fire E2048",
    );
}

#[test]
fn drop_let_projection_rejected_e2048() {
    // Impl lookup checks both the nominal and resolved receiver keys.
    // `let $f = p.a` on a Drop type produces E2048.
    assert_drop_partial_move_count(
        include_str!("fixtures/integration/drop_let_projection_rejected_e2048.ori"),
        1,
        "let-projection of a Drop-type field MUST fire exactly one E2048",
    );
}

#[test]
fn drop_match_nested_let_projection_in_arm_rejected_e2048() {
    // Negative pin: a `let $f = v.field` projection nested inside a match arm
    // body must be reached by the validator's FunctionSeq recursion.
    assert_drop_partial_move_count(
        include_str!(
            "fixtures/integration/drop_match_nested_let_projection_in_arm_rejected_e2048.ori"
        ),
        1,
        "nested let-projection inside a match arm MUST fire E2048",
    );
}

// E2048 nested-destructure recursion — a NESTED partial by-value destructure
// of a Drop-typed field is the same double-free / leak hazard as the top-level
// case, one level down. The OUTER pattern binds every field (whole-value
// consume at the top level), so the top-level field-count check alone does not
// flag it; the validator must recurse into nested struct/variant sub-patterns
// over Drop-typed fields. Matrix clamps the boundary from above (partial → fire)
// and below (bind-all nested → no fire).

#[test]
fn drop_match_nested_partial_struct_destructure_rejected_e2048() {
    // Negative pin: outer `Outer { inner: ... }` binds Outer's only field
    // (whole-value at the top level), but nested `Inner { x, .. }` binds 1 of 2
    // owned fields of the Drop-typed `inner` → partial move → E2048.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/drop_match_nested_partial_struct_destructure_rejected_e2048.ori"
    )));
    assert!(
        result.drop_partial_move_count() >= 1,
        "nested partial struct destructure of a Drop-typed field MUST fire E2048; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_nested_whole_value_struct_destructure_accepted() {
    // Positive pin: nested `Inner { x, y }` binds EVERY owned field of the
    // Drop-typed `inner` — whole-value consumption at every level → no E2048.
    assert_drop_partial_move_count(
        include_str!(
            "fixtures/integration/drop_match_nested_whole_value_struct_destructure_accepted.ori"
        ),
        0,
        "nested whole-value struct destructure MUST NOT fire E2048",
    );
}

#[test]
fn drop_match_nested_partial_in_enum_payload_rejected_e2048() {
    // Negative pin: outer variant `Wrap(inner)` binds its single payload field
    // (whole-value at the top level), but nested `Inner { x, .. }` partially
    // destructures the Drop-typed payload → E2048.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/drop_match_nested_partial_in_enum_payload_rejected_e2048.ori"
    )));
    assert!(
        result.drop_partial_move_count() >= 1,
        "nested partial destructure inside an enum payload MUST fire E2048; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_nested_partial_on_non_drop_inner_accepted() {
    // Negative-space pin: the E2048 axis is Drop-only. When the NESTED type is
    // NOT a Drop type, a nested partial destructure must NOT fire E2048 — even
    // though the outer type IS Drop. Recursion gates on the nested field's own
    // Drop status, not the outer's.
    assert_drop_partial_move_count(
        include_str!(
            "fixtures/integration/drop_match_nested_partial_on_non_drop_inner_accepted.ori"
        ),
        0,
        "nested partial destructure of a NON-Drop inner type MUST NOT fire E2048",
    );
}

// E2049 EVALUE_DROP_CONFLICT — runnable source-level enforcement at both
// non-derived registration surfaces. The conflict is pinned through the full
// lex→parse→typecheck pipeline (`check_source`).
//
// `check_source` does not load the prelude, so each source declares a local
// `trait Drop`. The Value marker is supplied via `#derive(Value)`, the
// parseable surface exercised by this integration harness. It co-fires E2033
// (Value-not-derivable), which these tests tolerate by counting E2049
// specifically.

impl CheckResult {
    /// Count `ValueDropConflict` (E2049) errors.
    fn value_drop_conflict_count(&self) -> usize {
        self.error_kinds()
            .iter()
            .filter(|k| matches!(k, TypeErrorKind::ValueDropConflict { .. }))
            .count()
    }
}

fn assert_value_drop_conflict_count(
    check: fn(&str) -> CheckResult,
    source: &'static str,
    expected: usize,
    context: &str,
) {
    let result = check(fixture_without_trailing_newline(source));
    assert_eq!(
        result.value_drop_conflict_count(),
        expected,
        "{context}; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn value_marker_with_drop_impl_fires_e2049_value_first() {
    // Value marker registered FIRST (type decl), Drop impl SECOND → E2049 at
    // the Drop-impl registration surface (Surface 2). Parse-error-tolerant:
    // the `#derive(Value)` form co-emits an E1016 parse diagnostic; E2049
    // still fires.
    assert_value_drop_conflict_count(
        check_source_allow_parse_errors,
        include_str!(
            "fixtures/integration/value_marker_with_drop_impl_fires_e2049_value_first.ori"
        ),
        1,
        "Value + Drop on the same type MUST fire exactly one E2049",
    );
}

#[test]
fn value_marker_without_drop_impl_no_e2049() {
    // Negative-space pin: the E2049 axis requires BOTH markers. A Value type
    // with NO Drop impl must NOT fire E2049 (it may still fire E2033 for the
    // non-derivable `#derive(Value)` form — that is a different axis).
    assert_value_drop_conflict_count(
        check_source_allow_parse_errors,
        include_str!("fixtures/integration/value_marker_without_drop_impl_no_e2049.ori"),
        0,
        "Value type without a Drop impl MUST NOT fire E2049",
    );
}

#[test]
fn drop_impl_without_value_marker_no_e2049() {
    // Negative-space pin: a Drop type with NO Value marker must NOT fire
    // E2049 (the conflict requires the Value marker too).
    assert_value_drop_conflict_count(
        check_source,
        include_str!("fixtures/integration/drop_impl_without_value_marker_no_e2049.ori"),
        0,
        "Drop type without the Value marker MUST NOT fire E2049",
    );
}

// Map-iterator lambda-parameter inference pins. `kvs.iter()` yields
// Iterator<(K, V)>, so `.map(...)` routes the `is_iterator()` branch of
// `unify_closure_param_with_iterator_elem`. Registry higher-order methods
// dispatch through the iterator receiver rather than a bare Map receiver.

#[test]
fn test_lambda_param_from_map_iter_iterator_elem_unchanged() {
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/lambda_param_from_map_iter_iterator_elem_unchanged.ori"
    )));
    assert!(
        !result.has_errors(),
        "map-iterator receiver must infer kv: (str, int); kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn test_lambda_param_from_iterator_receiver_unchanged_by_map_arm() {
    // Negative pin: a list-iterator receiver
    // still routes through the `is_iterator()` branch — widening the Map
    // arm to match `Tag::Iterator` would project map_key/map_value on a
    // non-Map idx and break this inference.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "fixtures/integration/lambda_param_from_iterator_receiver_unchanged_by_map_arm.ori"
    )));
    assert!(
        !result.has_errors(),
        "iterator receiver inference must be unchanged by the Map arm; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn s09_2_eager_direct_param_records_complete_instance() {
    let source =
        include_str!("fixtures/integration/s09_2_eager_direct_param_records_complete_instance.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "direct-param generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let instances = result.mono_instances_for("p1_id");
    assert_eq!(
        instances.len(),
        1,
        "eager direct-param `p1_id(x: 42)` must record exactly one instance, got: {instances:?}"
    );
    assert_eq!(instances[0].concrete_param_types, vec![Idx::INT]);
    assert_eq!(instances[0].concrete_return_type, Idx::INT);
}

// Pin 2 — eager indirect generic parameter (`T` occurs only inside
// `Pair<T, int>`) exercises `extract_indirect_scheme_var`.
#[test]
fn s09_2_eager_indirect_param_records_complete_instance() {
    let source = include_str!(
        "fixtures/integration/s09_2_eager_indirect_param_records_complete_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "indirect-param generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let instances = result.mono_instances_for("p2_firstof");
    assert_eq!(
        instances.len(),
        1,
        "eager indirect-param `p2_firstof` (T in Pair<T, int>) must record one instance, got: {instances:?}"
    );
    assert_eq!(
        instances[0].concrete_return_type,
        Idx::STR,
        "indirect T resolves to str"
    );
}

// Pin 3 — derived-Eq comparison on a GENERIC composite instantiation. Operator
// dispatch must publish the same concrete receiver-bearing method demand as an
// ordinary generic method call; ARC uses its receiver-qualified operator fact
// rather than a canonical call-dispatch entry to bind the realized body.
#[test]
fn s09_2_derived_method_on_generic_composite_typechecks_to_bool() {
    let source = include_str!(
        "fixtures/integration/s09_2_derived_method_on_generic_composite_typechecks_to_bool.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "derived-Eq generic-composite program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        body_ty,
        Idx::BOOL,
        "derived-Eq `p == q` on P3Pair<int, str> must resolve to bool; got {body_ty:?}"
    );
    let receiver = result
        .find_applied("P3Pair", &[Idx::INT, Idx::STR])
        .expect("concrete P3Pair<int, str> must exist");
    let instances = result.mono_instances_for("eq");
    assert_eq!(
        instances.len(),
        1,
        "generic derived Eq operator must publish one deduplicated method instance: {instances:?}"
    );
    let instance = instances[0];
    assert_eq!(instance.receiver_type, Some(receiver));
    assert_eq!(
        instance.impl_args,
        vec![
            crate::GenericArg::Type(Idx::INT),
            crate::GenericArg::Type(Idx::STR),
        ]
    );
    assert_eq!(instance.concrete_param_types, vec![receiver]);
    assert_eq!(instance.concrete_return_type, Idx::BOOL);
}

#[test]
fn generic_derived_methods_share_exact_applied_self_identity() {
    let source = include_str!(
        "fixtures/integration/generic_derived_methods_share_exact_applied_self_identity.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic derived methods must resolve without poison; kinds: {:?}",
        result.error_kinds()
    );

    let clone_ty = result.function_body_type("p3_clone").unwrap();
    let expected = result
        .find_applied("P3Box", &[Idx::INT])
        .expect("concrete P3Box<int> must exist");
    assert_eq!(clone_ty, expected);
    let expected_str = result
        .find_applied("P3Box", &[Idx::STR])
        .expect("concrete P3Box<str> must exist");
    assert_eq!(
        result.function_body_type("p3_clone_str"),
        Some(expected_str)
    );
    assert_eq!(result.function_body_type("p3_hash"), Some(Idx::INT));

    let type_param = result.interner.intern("T");
    for accepted in &result.result.typed.accepted_derives {
        assert_eq!(result.tag(accepted.owner_type), Tag::Applied);
        assert_eq!(
            result.pool.applied_name(accepted.owner_type),
            result.interner.intern("P3Box")
        );
        let args = result.pool.applied_args(accepted.owner_type);
        assert_eq!(args.len(), 1);
        assert_eq!(result.tag(args[0]), Tag::Named);
        assert_eq!(result.pool.named_name(args[0]), type_param);
        assert_eq!(accepted.signature.type_params, vec![type_param]);
        assert!(accepted
            .signature
            .param_types
            .iter()
            .all(|&param| param == accepted.owner_type));
        if accepted.trait_kind == ori_ir::DerivedTrait::Clone {
            assert_eq!(accepted.signature.return_type, accepted.owner_type);
        }
    }
}

#[test]
fn concrete_generic_bound_seeds_exact_derived_method_body() {
    let source = include_str!(
        "fixtures/integration/concrete_generic_bound_seeds_exact_derived_method_body.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "bound-method generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );

    let receiver = result
        .find_applied("BoundBox", &[Idx::INT])
        .expect("concrete BoundBox<int> must exist");
    let accepted = result
        .result
        .typed
        .accepted_derives
        .iter()
        .find(|accepted| accepted.trait_kind == ori_ir::DerivedTrait::Debug)
        .expect("Debug derive must be accepted");
    let producer = crate::MethodProducer::Derived(accepted.id);
    let instances: Vec<_> = result
        .result
        .typed
        .mono_instances
        .iter()
        .filter(|instance| {
            instance.method_producer.as_ref() == Some(&producer)
                && instance.receiver_type == Some(receiver)
        })
        .collect();

    assert_eq!(
        instances.len(),
        1,
        "the concrete bound must publish one exact derived Debug body: {instances:?}"
    );
    assert_eq!(
        instances[0].impl_args,
        vec![crate::GenericArg::Type(Idx::INT)]
    );
    assert!(result
        .result
        .typed
        .derived_call_plans
        .iter()
        .any(|plan| plan.derived == accepted.id && plan.binder_substitutions == [Idx::INT]));
}

// Pin 4 — builtin Duration ctor (`Duration.from_seconds`). The factory family
// is a non-generic builtin (`MethodKind::Associated` in `ori_registry`, not in
// `impl_sigs`), so a recorded MonoInstance would be skipped by
// `collect_mono_functions` — the family is delivered codegen-direct via
// `try_emit_builtin_associated`, not by the mono recorder. This pin holds the
// typeck-layer contract: the call type-checks and its return resolves to
// `Duration`. AOT execution also validates the runtime value.
#[test]
fn s09_2_builtin_duration_ctor_typechecks_to_duration() {
    let source =
        include_str!("fixtures/integration/s09_2_builtin_duration_ctor_typechecks_to_duration.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Duration ctor program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        body_ty,
        Idx::DURATION,
        "Duration.from_seconds(s: 5) must resolve to Duration; got {body_ty:?}"
    );
}

// Pin 5 — builtin iterator/DEI method chain (`.iter().rev().collect()`).
// Builtin iterator methods take the `ReceiverDispatch::Return` arm and are
// delivered codegen-direct (the `"Iterator"` / DEI re-keyed dispatch in
// `ori_llvm` `codegen/arc_emitter/builtins/mod.rs` reaches the existing
// `emit_iter_*` emitters), NOT by the mono recorder — so an instance-recorded
// assertion is the wrong contract (the factory family proved this). This pin
// holds the durable typeck-layer contract: the chain type-checks and its
// `.collect()` result resolves to `[int]`. AOT execution also validates the
// runtime value.
#[test]
fn s09_2_iterator_method_typechecks_to_list() {
    let source = include_str!("fixtures/integration/s09_2_iterator_method_typechecks_to_list.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "iterator .rev().collect() program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        result.tag(body_ty),
        Tag::List,
        "[1, 2, 3].iter().rev().collect() must resolve to a list; got {body_ty:?}"
    );
    assert_eq!(
        result.pool.list_elem(body_ty),
        Idx::INT,
        "the collected list element must be int; got {body_ty:?}"
    );
}

// Pin 6 — deferred-route NEGATIVE clamp. A generic-calling-generic
// (`wrap6<U>` body calls `id6(x: y)` while `y: U` is still a variable) MUST
// route to `record_deferred_mono_call`, never record a bogus EAGER instance
// whose concrete types still carry a `Tag::Var`.
#[test]
fn s09_2_deferred_route_records_no_var_typed_instance() {
    let source =
        include_str!("fixtures/integration/s09_2_deferred_route_records_no_var_typed_instance.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic-calling-generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    // No recorded instance may carry an unresolved Var in its concrete types —
    // the deferred path must NOT leak a half-resolved eager instance.
    for inst in result.mono_instances_all() {
        for &pt in &inst.concrete_param_types {
            assert_ne!(
                result.tag(pt),
                Tag::Var,
                "deferred route leaked a Var-typed concrete param into instance {inst:?}"
            );
        }
        assert_ne!(
            result.tag(inst.concrete_return_type),
            Tag::Var,
            "deferred route leaked a Var-typed concrete return into instance {inst:?}"
        );
    }
}

// Generic STRUCT body materialization. `Applied(P3Pair,[int,str])` must resolve
// through `Pool.resolutions` to a concrete `Tag::Struct` whose field types are the
// concrete args (`int`/`str`), NOT the generic param refs. Without materialization
// the `Applied` carries no resolution (codegen reads the generic field).
#[test]
fn generic_struct_applied_resolves_to_concrete_body() {
    let source = include_str!(
        "fixtures/integration/s09_2_derived_method_on_generic_composite_typechecks_to_bool.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic-struct program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let applied = result
        .find_applied("P3Pair", &[Idx::INT, Idx::STR])
        .expect("Applied(P3Pair, [int, str]) must exist in the pool");
    let concrete = result
        .pool
        .resolve(applied)
        .expect("Applied(P3Pair, [int, str]) must resolve to a concrete body");
    assert_eq!(
        result.tag(concrete),
        Tag::Struct,
        "the materialized resolution must be a concrete Struct; got {concrete:?}"
    );
    let fields = result.pool.struct_fields(concrete);
    let field_tys: Vec<Idx> = fields.iter().map(|&(_, ty)| ty).collect();
    assert_eq!(
        field_tys,
        vec![Idx::INT, Idx::STR],
        "materialized struct fields must be concrete int/str, not the generic params; got {field_tys:?}"
    );
}

// Struct-field NEGATIVE behavioral pin. The materialized struct field type must
// NOT be a `Tag::Named` generic-param ref — without materialization the field
// stays the declared `Tag::Named(A)`/`Named(B)`; materialized it is concrete.
#[test]
fn materialized_struct_field_is_not_generic_param() {
    let source = include_str!(
        "fixtures/integration/s09_2_derived_method_on_generic_composite_typechecks_to_bool.ori"
    );
    let result = check_source(source);
    let applied = result
        .find_applied("P3Pair", &[Idx::INT, Idx::STR])
        .expect("Applied(P3Pair, [int, str]) must exist");
    let concrete = result
        .pool
        .resolve(applied)
        .expect("Applied must resolve to a concrete body post-fix");
    for (fname, fty) in result.pool.struct_fields(concrete) {
        assert_ne!(
            result.tag(fty),
            Tag::Named,
            "field {fname:?} kept a generic-param Named ref after materialization: {fty:?}"
        );
    }
}

// Enum variant-payload NEGATIVE behavioral pin. `Either<int,str>` `R`-variant
// payload Idx must be the concrete `str`, NOT the declared generic param
// `Tag::Named(B)`. Pins behavior (the payload Idx value), not symbol existence.
#[test]
fn generic_enum_r_payload_resolves_to_concrete() {
    let source =
        include_str!("fixtures/integration/generic_enum_r_payload_resolves_to_concrete.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic-enum program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let applied = result
        .find_applied("Either", &[Idx::INT, Idx::STR])
        .expect("Applied(Either, [int, str]) must exist in the pool");
    let concrete = result
        .pool
        .resolve(applied)
        .expect("Applied(Either, [int, str]) must resolve to a concrete enum body");
    assert_eq!(
        result.tag(concrete),
        Tag::Enum,
        "the materialized resolution must be a concrete Enum; got {concrete:?}"
    );
    let variants = result.pool.enum_variants(concrete);
    // R is the second variant; its single payload must be concrete `str`.
    let r_payload = variants
        .iter()
        .find(|(vname, _)| *vname == result.interner.intern("R"))
        .map(|(_, payloads)| payloads.clone())
        .expect("the R variant must be present");
    assert_eq!(
        r_payload,
        vec![Idx::STR],
        "R payload must materialize to concrete str, not the generic param; got {r_payload:?}"
    );
}

#[test]
fn s09_2_deferred_resolve_produces_concrete_instance() {
    let source =
        include_str!("fixtures/integration/s09_2_deferred_resolve_produces_concrete_instance.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "deferred-resolve program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let id_instances = result.mono_instances_for("p7_id");
    assert!(
        id_instances
            .iter()
            .any(|m| m.concrete_param_types == vec![Idx::INT]
                && m.concrete_return_type == Idx::INT),
        "deferred `p7_id` (called only from generic `p7_wrap`) must resolve to a \
         concrete p7_id<int> instance, got: {id_instances:?}"
    );
}

#[test]
fn s09_2_dei_consumer_typechecks_to_option() {
    let source = include_str!("fixtures/integration/s09_2_dei_consumer_typechecks_to_option.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "iterator .last() program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        result.tag(body_ty),
        Tag::Option,
        "[1, 2, 3].iter().last() must resolve to an Option; got {body_ty:?}"
    );
    assert_eq!(
        result.pool.option_inner(body_ty),
        Idx::INT,
        "the Option element from .last() must be int; got {body_ty:?}"
    );
}
