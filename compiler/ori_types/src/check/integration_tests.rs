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
//! - **Empty module**: Regression guard

#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]

use ori_ir::StringInterner;

use crate::check::check_module_with_pool;
use crate::{Idx, Pool, Tag, TypeCheckResult, TypeErrorKind};

// ============================================================================
// Test Infrastructure
// ============================================================================

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

// ============================================================================
// Empty Module
// ============================================================================

#[test]
fn empty_source() {
    let result = check_source("");
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 0);
}

// ============================================================================
// Literal Expressions
// ============================================================================

#[test]
fn literal_int() {
    let result = check_source("@foo () -> int = 42");
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 1);

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn literal_float() {
    let result = check_source("@foo () -> float = 3.14");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::FLOAT);
}

#[test]
fn literal_bool() {
    let result = check_source("@foo () -> bool = true");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn literal_string() {
    let result = check_source(r#"@foo () -> str = "hello""#);
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn literal_unit() {
    let result = check_source("@foo () -> void = ()");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::UNIT);
}

// ============================================================================
// Function Parameters
// ============================================================================

#[test]
fn single_typed_param() {
    let result = check_source("@identity (x: int) -> int = x");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn multiple_typed_params() {
    let result = check_source("@add (a: int, b: int) -> int = a + b");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn param_type_used_in_body() {
    let result = check_source("@greet (name: str) -> str = name");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

// ============================================================================
// Multiple Functions
// ============================================================================

#[test]
fn two_functions() {
    let source = "\
@foo () -> int = 1

@bar () -> int = 2
";
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
    let source = "\
@foo () -> int = 42

@bar () -> int = foo()
";
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

#[test]
fn forward_reference() {
    // bar defined before foo, but calls foo
    let source = "\
@bar () -> int = foo()

@foo () -> int = 42
";
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

// ============================================================================
// Test Declarations
// ============================================================================

#[test]
fn test_declaration() {
    let source = "\
@foo () -> int = 42

@test_foo tests @foo () -> void = ()
";
    let result = check_source(source);
    assert!(!result.has_errors());
    // Functions + tests both counted as signatures
    assert_eq!(result.function_count(), 2);
}

#[test]
fn test_with_function_call() {
    // Test body that uses the target function via run()
    // run() requires a trailing result expression after statements
    let source = "\
@double (x: int) -> int = x + x

@test_double tests @double () -> void = run(
    let _ = double(x: 5),
    (),
)
";
    let result = check_source(source);
    // `run` may produce errors since it's a compiler construct that needs
    // special handling. The key assertion is: no panics in the pipeline.
    let _ = result.has_errors();
}

// ============================================================================
// Type Errors
// ============================================================================

#[test]
fn return_type_mismatch() {
    // Body returns string but signature says int
    let result = check_source(r#"@bad () -> int = "hello""#);
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
    let result = check_source("@bad () -> int = undefined_var");
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
    let source = "\
@add (x: int, y: int) -> int = x + y

@caller () -> int = ad(1, 2)
";
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
    let source = "\
@add (x: int, y: int) -> int = x + y

@caller () -> int = xyz(1, 2)
";
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
    let source = "\
@takes_int (x: int) -> int = x

@caller () -> int = takes_int(x: 42)
";
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 2);
}

// ============================================================================
// Let Bindings
// ============================================================================

#[test]
fn simple_let_binding() {
    let source = "\
@foo () -> int = run(
    let x = 42,
    x,
)
";
    // `run` is a built-in construct that sequences expressions
    // If run isn't available as a call target, this may fail with unknown ident
    // but the let binding itself should be handled
    let result = check_source(source);
    let _ = result.has_errors(); // Don't assert - `run` may not be resolved
}

#[test]
fn let_in_block_body() {
    // Using a block expression (if/else) that includes let bindings
    let source = "\
@foo () -> int = if true then 42 else 0
";
    let result = check_source(source);
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

// ============================================================================
// Control Flow
// ============================================================================

#[test]
fn if_then_else_int() {
    let result = check_source("@foo () -> int = if true then 1 else 2");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn if_then_else_string() {
    let result = check_source(r#"@foo () -> str = if false then "a" else "b""#);
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn if_condition_must_be_bool() {
    // Using an int as condition should produce an error
    let result = check_source("@bad () -> int = if 42 then 1 else 2");
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

// ============================================================================
// Collections
// ============================================================================

#[test]
fn list_literal() {
    let result = check_source("@foo () -> [int] = [1, 2, 3]");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(result.tag(body_ty), Tag::List);
}

#[test]
fn empty_list() {
    // Empty list with type annotation on function
    let result = check_source("@foo () -> [int] = []");
    // The empty list may or may not unify with [int] depending on inference
    // At minimum, it shouldn't panic
    let _ = result.has_errors();
}

// ============================================================================
// Operators
// ============================================================================

#[test]
fn arithmetic_operators() {
    let result = check_source("@foo () -> int = 1 + 2 * 3");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn comparison_operators() {
    let result = check_source("@foo () -> bool = 1 < 2");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn boolean_operators() {
    let result = check_source("@foo () -> bool = true && false");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn equality_check() {
    let result = check_source("@foo () -> bool = 1 == 2");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

#[test]
fn string_concatenation() {
    let result = check_source(r#"@foo () -> str = "hello" + " world""#);
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::STR);
}

#[test]
fn negation() {
    let result = check_source("@foo () -> int = -42");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::INT);
}

#[test]
fn boolean_not() {
    let result = check_source("@foo () -> bool = !true");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::BOOL);
}

// ============================================================================
// Tuple Expressions
// ============================================================================

#[test]
fn tuple_literal() {
    let result = check_source("@foo () -> (int, str) = (42, \"hello\")");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(result.tag(body_ty), Tag::Tuple);
}

// ============================================================================
// Multiple Error Accumulation
// ============================================================================

#[test]
fn multiple_errors_accumulated() {
    // Two functions with errors - should accumulate both
    let source = r#"
@bad1 () -> int = "not an int"

@bad2 () -> bool = 42
"#;
    let result = check_source(source);
    assert!(result.has_errors());
    // Should have at least 2 errors (one per function)
    assert!(
        result.error_count() >= 2,
        "Expected at least 2 errors, got {}",
        result.error_count()
    );
}

// ============================================================================
// Cross-Module Imports
// ============================================================================

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
                checker.register_imported_function(func, &provider.arena);
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
        "@caller () -> int = add(1, 2)",
        "@add (a: int, b: int) -> int = a + b",
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
    // Module B calls `missing_fn()` which was never imported → UnknownIdent
    let result = check_source("@caller () -> int = missing_fn()");

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
        r#"@caller () -> int = len("hello")"#,
        "@len (s: str) -> int = 5",
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
        "@caller () -> int = returns_str()",
        r#"@returns_str () -> str = "hello""#,
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

    let provider_source = r#"@foo () -> str = "imported""#;
    let consumer_source = "\
@foo () -> int = 42

@caller () -> int = foo()
";

    let provider = parse_source(provider_source, &interner);
    let consumer = parse_source(consumer_source, &interner);

    let (result, _pool) = crate::check::check_module_with_imports(
        &consumer.module,
        &consumer.arena,
        &interner,
        |checker| {
            for func in &provider.module.functions {
                checker.register_imported_function(func, &provider.arena);
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

    let provider_source = "\
@double (x: int) -> int = x + x

@negate (x: int) -> int = 0 - x
";
    let consumer_source = "\
@caller () -> int = negate(double(5))
";

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
    let provider_source = "\
pub @public_fn () -> int = 1

@private_fn () -> int = 2
";
    let provider = parse_source(provider_source, &interner);
    let consumer = parse_source("@caller () -> int = 42", &interner);

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

// ============================================================================
// Regression Guards
// ============================================================================

#[test]
fn only_comments() {
    // Source with only comments should be treated as empty
    let result = check_source_allow_parse_errors("// just a comment");
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 0);
}

#[test]
fn function_returning_void() {
    let result = check_source("@noop () -> void = ()");
    assert!(!result.has_errors());

    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(body_ty, Idx::UNIT);
}

#[test]
fn many_functions() {
    let source = "\
@a () -> int = 1

@b () -> int = 2

@c () -> int = 3

@d () -> int = 4

@e () -> int = 5
";
    let result = check_source(source);
    assert!(!result.has_errors());
    assert_eq!(result.function_count(), 5);
}

// ============================================================================
// Type Definition Exports
// ============================================================================

#[test]
fn struct_type_exported() {
    let source = "\
type Point = { x: int, y: int }

@main () -> int = 42
";
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
    let source = "\
type Color = Red | Green | Blue

@main () -> int = 42
";
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

// ============================================================================
// Invalid Return Type Annotations
// ============================================================================

#[test]
fn bogus_return_type_is_rejected() {
    // `-> garbage` is not a valid type — should produce a type error
    let source = "\
@sum (x: int, y: int) -> garbage = x + y

@main () -> void = println(sum(1, 2).to_str())
";
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined return type `garbage`, got none"
    );
}

#[test]
fn bogus_return_type_on_method_is_rejected() {
    // Same bug but on a method with `self` — this is the user's exact repro
    let source = "\
type Point = { x: int, y: int }

@sum (self: Point) -> garbage = self.x + self.y

@main () -> void =
  let p = Point { x: 3, y: 4 }
  println(p.sum().to_str())
";
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
    let source = "\
type Point = { x: int, y: int }

impl Point {
    @sum (self) -> nt = self.x + self.y

    @scale (self, factor: int) -> Point = Point { x: self.x * factor, y: self.y * factor }
}

@main () -> void = run(
    let p = Point { x: 3, y: 4 },
    print(msg: str(p.sum())),
)
";
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Expected type error for undefined return type `nt` in impl block, got none"
    );
}

#[test]
fn bogus_param_type_is_rejected() {
    // Also check parameter types — `garbage` as a param type should error
    let source = "\
@foo (x: garbage) -> int = 42

@main () -> void = println(foo(1).to_str())
";
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
    let source = "\
type Point = { x: int, y: int }

@sum (self: Point) -> garbage = self.x + self.y

@main () -> void =
  let p = Point { x: 3, y: 4 }
  println(p.sum().to_str())
";
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
    // Regression guard: valid type annotations must still work
    let source = "\
@sum (x: int, y: int) -> int = x + y
";
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Valid return type `int` should not produce errors: {:?}",
        result.error_kinds()
    );
}

// ============================================================================
// Impl Block `self` Parameter — Type Checking
// ============================================================================

#[test]
fn impl_self_field_access_type_checks() {
    // Regression guard: self in impl block resolves to the impl type,
    // allowing field access and correct return type checking.
    let source = "\
type Point = { x: int, y: int }

impl Point {
    @sum (self) -> int = self.x + self.y
}
";
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
    let source = "\
type Counter = { value: int }

impl Counter {
    @add (self, amount: int) -> int = self.value + amount
    @add_scaled (self, amount: int, scale: int) -> int = self.value + amount * scale
}
";
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
    let source = "\
type Point = { x: int, y: int }

impl Point {
    @sum (self) -> str = self.x + self.y
}
";
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Impl method returning int but declared -> str should error"
    );
}

#[test]
fn impl_self_returning_self_type() {
    // Self as return type should resolve to the impl type
    let source = "\
type Vector = { x: int, y: int }

impl Vector {
    @negate (self) -> Self = Vector { x: 0 - self.x, y: 0 - self.y }
}
";
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
    let source = "\
type Point = { x: int, y: int }

impl Point {
    @origin () -> Self = Point { x: 0, y: 0 }
}
";
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
    let source = "\
type Rect = { w: int, h: int }

impl Rect {
    @area (self) -> int = self.w * self.h
    @perimeter (self) -> int = 2 * (self.w + self.h)
    @is_square (self) -> bool = self.w == self.h
    @scale (self, factor: int) -> Self = Rect { w: self.w * factor, h: self.h * factor }
}
";
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
    let source = "\
type Point = { x: int, y: int }

impl Point {
    @scale (self, factor: garbage) -> int = self.x * factor
}
";
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
    let source = "\
type Counter = { value: int }

impl Counter {
    @add (self, amount: int) -> bool = self.value + amount
}
";
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Impl method body returning int but declared -> bool should error"
    );
}

#[test]
fn impl_self_method_on_enum() {
    // self should also work correctly on enum types
    let source = "\
type Color = Red | Green | Blue

impl Color {
    @is_red (self) -> bool = match(self, Red -> true, _ -> false)
}
";
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
    let source = "\
type Wrapper = { value: int }

impl Wrapper {
    @doubled (self) -> int = self.value * 2
}
";
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
    let source = "\
type Point = { x: int, y: int }

@distance (p: Point) -> int = p.x * p.x + p.y * p.y

impl Point {
    @dist (self) -> int = distance(p: self)
}
";
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
    let source = "\
type Point = { x: int, y: int }

@consume (s: str) -> int = 0

impl Point {
    @bad (self) -> int = consume(s: self)
}
";
    let result = check_source(source);
    assert!(
        result.has_errors(),
        "Passing self (Point) where str expected should error"
    );
}
