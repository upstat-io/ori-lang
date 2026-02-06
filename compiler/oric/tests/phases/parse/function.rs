//! Parser tests for function definitions.
//!
//! Validates that the parser correctly enforces:
//! - Mandatory return type annotations on functions and tests
//! - Grammar: `function = "@" identifier [ generics ] clause_params "->" type ... "=" expression`

use crate::common::{parse_err, parse_ok};

// =============================================================================
// Mandatory return type — functions
// =============================================================================

#[test]
fn test_function_requires_return_type() {
    parse_err("@foo (x: int) = x * 2", "expected ->");
}

#[test]
fn test_function_with_return_type_parses() {
    let output = parse_ok("@foo (x: int) -> int = x * 2");
    assert_eq!(output.module.functions.len(), 1);
}

#[test]
fn test_function_no_params_requires_return_type() {
    parse_err("@bar () = 42", "expected ->");
}

#[test]
fn test_function_no_params_with_return_type_parses() {
    let output = parse_ok("@bar () -> int = 42");
    assert_eq!(output.module.functions.len(), 1);
}

// =============================================================================
// Mandatory return type — free-floating tests
// =============================================================================

#[test]
fn test_free_floating_test_requires_return_type() {
    parse_err("@test_something () = ()", "expected ->");
}

#[test]
fn test_free_floating_test_with_return_type_parses() {
    let output = parse_ok("@test_something () -> void = ()");
    assert_eq!(output.module.tests.len(), 1);
}

// =============================================================================
// Mandatory return type — targeted tests
// =============================================================================

#[test]
fn test_targeted_test_requires_return_type() {
    parse_err(
        "@foo () -> int = 42\n@test_foo tests @foo () = ()",
        "expected ->",
    );
}

#[test]
fn test_targeted_test_with_return_type_parses() {
    let output = parse_ok("@foo () -> int = 42\n@test_foo tests @foo () -> void = ()");
    assert_eq!(output.module.functions.len(), 1);
    assert_eq!(output.module.tests.len(), 1);
}
