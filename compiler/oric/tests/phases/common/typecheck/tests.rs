use super::*;

#[test]
fn test_typecheck_source_simple() {
    let result = typecheck_source("@add(a: int, b: int) -> int = a + b");
    assert!(!result.has_errors());
}

#[test]
fn test_typecheck_ok_succeeds() {
    let _result = typecheck_ok("@main () -> int = 42");
}

#[test]
#[should_panic(expected = "Expected successful type check")]
fn test_typecheck_ok_panics_on_error() {
    typecheck_ok("@main () -> int = \"not an int\"");
}

#[test]
fn test_typecheck_err_catches_mismatch() {
    typecheck_err("@main () -> int = \"hello\"", "mismatch");
}

// Regression: let bindings directly in function body (no run() wrapper)
// Previously crashed with type_interner index out of bounds.

#[test]
fn test_let_binding_in_main_body() {
    typecheck_ok("@main () -> void = let x: int = 42");
}

#[test]
fn test_let_binding_str_in_main_body() {
    typecheck_ok("@main () -> void = let x: str = \"hello\"");
}

#[test]
fn test_let_binding_inferred_in_main_body() {
    typecheck_ok("@main () -> void = let x = 42");
}

#[test]
fn test_let_binding_float_in_main_body() {
    typecheck_ok("@main () -> void = let x: float = 3.14");
}

#[test]
fn test_let_binding_bool_in_main_body() {
    typecheck_ok("@main () -> void = let x: bool = true");
}

#[test]
fn test_let_binding_in_regular_function_body() {
    typecheck_ok("@f () -> void = let x: int = 42");
}
