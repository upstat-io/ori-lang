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

// Regression: let bindings directly in function body (no run() wrapper)
// Previously crashed with type_interner index out of bounds.

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

// Regression: type-checking an `impl Type: Trait` whose trait is NOT registered
// (unknown trait name, OR a prelude trait when no prelude is loaded — this harness
// loads none) must emit a clean unresolved-trait diagnostic, NEVER panic. Pre-fix,
// `validate_assoc_types` hit `debug_assert!(false)` (compiler/ori_types/src/check/
// registration/impls.rs:445) and ICE'd; the fix adds a pre-block guard in
// `register_impl` that emits E2003 and skips the trait-impl validation block.
// See: bug-tracker/plans/BUG-02-034/.

#[test]
fn impl_with_unregistered_trait_emits_diagnostic_not_ice() {
    // Negative pin: the unresolved-trait diagnostic must name the offending trait.
    typecheck_err(
        "type Foo = { x: int }\nimpl Foo: SomeUnknownTrait {\n@m (self) -> int = self.x;\n}\n",
        "SomeUnknownTrait",
    );
}

#[test]
fn impl_drop_without_prelude_registration_emits_diagnostic_not_ice() {
    // Drop is unregistered in this no-prelude harness → must be a clean
    // unresolved-trait error, never the debug_assert ICE. (With the prelude loaded
    // + the Drop declaration, `impl Type: Drop` type-checks — see the spec test.)
    typecheck_err(
        "type Resource = { id: int }\nimpl Resource: Drop {\n@drop (self) -> void = ();\n}\n",
        "Drop",
    );
}

#[test]
fn impl_eq_without_prelude_emits_diagnostic_not_ice() {
    // Missing-prelude path: `impl T: Eq` with no prelude resolved is the original
    // repro shape — clean diagnostic, not panic.
    typecheck_err(
        "type Point = { x: int, y: int }\nimpl Point: Eq {\n@equals (self, other: Self) -> bool = self.x == other.x;\n}\n",
        "Eq",
    );
}
