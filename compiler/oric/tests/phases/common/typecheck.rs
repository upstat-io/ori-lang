//! Type check phase test utilities.
//!
//! Provides helpers for testing the type checker.

use ori_ir::StringInterner;
use ori_lexer::lex;
use ori_parse::parse;
use ori_typeck::{type_check, type_check_with_source, TypeCheckError, TypedModule};

/// Type check source code.
///
/// Handles lexing, parsing, and type checking in one step. Returns the
/// `TypedModule` which contains type information and any errors.
///
/// # Example
///
/// ```ignore
/// let result = typecheck_source("@add(a: int, b: int) -> int = a + b");
/// assert!(result.errors.is_empty());
/// ```
pub fn typecheck_source(source: &str) -> TypedModule {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    type_check_with_source(&parsed, &interner, source.to_string())
}

/// Type check source and assert it succeeds.
///
/// Panics if there are parse errors or type check errors.
///
/// # Example
///
/// ```ignore
/// let typed = typecheck_ok("@main () -> int = 42");
/// ```
pub fn typecheck_ok(source: &str) -> TypedModule {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);

    assert!(
        !parsed.has_errors(),
        "Parse errors before type checking:\n{:?}\nSource:\n{source}",
        parsed.errors
    );

    let typed = type_check(&parsed, &interner);

    assert!(
        typed.errors.is_empty(),
        "Expected successful type check, but got errors:\n{}\nSource:\n{source}",
        format_typecheck_errors(&typed.errors)
    );

    typed
}

/// Type check source and assert it fails with an error containing the expected message.
///
/// Panics if type checking succeeds or if no error contains the expected substring.
///
/// # Example
///
/// ```ignore
/// typecheck_err("let x: int = \"hello\"", "type mismatch");
/// ```
pub fn typecheck_err(source: &str, expected_error: &str) {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);

    // Parse errors are acceptable - they might be what we're testing
    if parsed.has_errors() {
        let has_expected = parsed
            .errors
            .iter()
            .any(|e| format!("{e:?}").contains(expected_error));
        if has_expected {
            return;
        }
    }

    let typed = type_check(&parsed, &interner);

    assert!(
        !typed.errors.is_empty() || parsed.has_errors(),
        "Expected type check error containing '{expected_error}', but checking succeeded.\nSource: {source}"
    );

    let has_expected = typed
        .errors
        .iter()
        .any(|e| format!("{e:?}").contains(expected_error));

    assert!(
        has_expected,
        "Expected type check error containing '{expected_error}', but got:\n{}\nSource:\n{source}",
        format_typecheck_errors(&typed.errors)
    );
}

/// Format type check errors for display in test failure messages.
fn format_typecheck_errors(errors: &[TypeCheckError]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for (i, error) in errors.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        let _ = write!(result, "Error {}: {error:?}", i + 1);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typecheck_source_simple() {
        let typed = typecheck_source("@add(a: int, b: int) -> int = a + b");
        assert!(typed.errors.is_empty());
    }

    #[test]
    fn test_typecheck_ok_succeeds() {
        let _typed = typecheck_ok("@main () -> int = 42");
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
}
