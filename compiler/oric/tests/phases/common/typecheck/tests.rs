use super::*;

#[test]
fn test_typecheck_source_simple() {
    let result = typecheck_source("@add(a: int, b: int) -> int = a + b;");
    assert!(!result.has_errors());
}

#[test]
fn test_typecheck_ok_succeeds() {
    let _result = typecheck_ok("@main () -> int = 42;");
}

#[test]
#[should_panic(expected = "Expected successful type check")]
fn test_typecheck_ok_panics_on_error() {
    typecheck_ok("@main () -> int = \"not an int\";");
}

#[test]
fn test_typecheck_err_catches_mismatch() {
    typecheck_err("@main () -> int = \"hello\";", "mismatch");
}

// Regression: let bindings directly in a function body (no run() wrapper) must type-check.

#[test]
fn test_let_binding_in_main_body() {
    typecheck_ok("@main () -> void = let x: int = 42;");
}

#[test]
fn test_let_binding_str_in_main_body() {
    typecheck_ok("@main () -> void = let x: str = \"hello\";");
}

#[test]
fn test_let_binding_inferred_in_main_body() {
    typecheck_ok("@main () -> void = let x = 42;");
}

#[test]
fn test_let_binding_float_in_main_body() {
    typecheck_ok("@main () -> void = let x: float = 3.14;");
}

#[test]
fn test_let_binding_bool_in_main_body() {
    typecheck_ok("@main () -> void = let x: bool = true;");
}

#[test]
fn test_let_binding_in_regular_function_body() {
    typecheck_ok("@f () -> void = let x: int = 42;");
}

// Regression: an `impl Type: Trait` naming an unregistered trait (unknown name, or a
// prelude trait under this no-prelude harness) must emit a clean E2003, never ICE.
// `typecheck_err` matches the interner-free `message()` — assert the fixed substring.

#[test]
fn impl_with_unregistered_trait_emits_diagnostic_not_ice() {
    // Negative pin: an unknown trait name in impl position → clean E2003, not ICE.
    typecheck_err(
        "type Foo = { x: int }\nimpl Foo: SomeUnknownTrait {\n@m (self) -> int = self.x;\n}\n",
        "unresolved trait",
    );
}

#[test]
fn impl_drop_without_prelude_registration_emits_diagnostic_not_ice() {
    // Negative pin: Drop unregistered in this no-prelude harness → asserts the clean
    // "unresolved trait" diagnostic, never the debug_assert ICE. (With the prelude +
    // the Drop declaration, `impl Type: Drop` type-checks — see the spec test.)
    typecheck_err(
        "type Resource = { id: int }\nimpl Resource: Drop {\n@drop (self) -> void = ();\n}\n",
        "unresolved trait",
    );
}

#[test]
fn impl_eq_without_prelude_emits_diagnostic_not_ice() {
    // Missing-prelude path: `impl T: Eq` with no prelude resolved is the original
    // repro shape — clean diagnostic, not panic.
    typecheck_err(
        "type Point = { x: int, y: int }\nimpl Point: Eq {\n@equals (self, other: Self) -> bool = self.x == other.x;\n}\n",
        "unresolved trait",
    );
}

/// L12 production-path pin: drive real source through parse -> typeck -> the
/// production renderer (`render_type_errors`) and assert the E2014 `= help:`
/// suggestion resolves the interned `Name` (renders `uses Suspend`) and does
/// NOT leak the raw `Name(shard=.., local=..)` Debug form.
#[test]
fn missing_capability_help_renders_resolved_name_via_production_path() {
    use ori_ir::StringInterner;
    use ori_lexer::lex;
    use ori_parse::parse;
    use ori_types::{check_module_with_imports, render_type_errors};

    let source = "@needs () -> int uses Suspend = 42\n\n@main () -> int = needs()\n";
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    let (result, pool) =
        check_module_with_imports(&parsed.module, &parsed.arena, &interner, |_c| {});

    assert!(result.has_errors(), "expected the E2014 capability error");
    let diags = render_type_errors(result.errors(), &pool, &interner);
    let help_texts: Vec<String> = diags.iter().flat_map(|d| d.suggestions.clone()).collect();

    assert!(
        help_texts.iter().any(|s| s.contains("uses Suspend")),
        "help suggestion should render `uses Suspend`: {help_texts:?}"
    );
    assert!(
        !help_texts.iter().any(|s| s.contains("Name(")),
        "help suggestion must not leak raw Name Debug form: {help_texts:?}"
    );
}
