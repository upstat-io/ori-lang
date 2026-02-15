//! Type check phase test utilities.
//!
//! Provides helpers for testing the type checker.

use ori_ir::StringInterner;
use ori_lexer::lex;
use ori_parse::parse;
use ori_types::{TypeCheckError, TypeCheckResult};

/// Type check source code.
///
/// Handles lexing, parsing, and type checking in one step. Returns the
/// `TypeCheckResult` which contains type information and any errors.
///
/// # Example
///
/// ```ignore
/// let result = typecheck_source("@add(a: int, b: int) -> int = a + b");
/// assert!(!result.has_errors());
/// ```
pub fn typecheck_source(source: &str) -> TypeCheckResult {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    let (result, _pool) = ori_types::check_module_with_imports(
        &parsed.module,
        &parsed.arena,
        &interner,
        |_checker| {},
    );
    result
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
pub fn typecheck_ok(source: &str) -> TypeCheckResult {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);

    assert!(
        !parsed.has_errors(),
        "Parse errors before type checking:\n{:?}\nSource:\n{source}",
        parsed.errors
    );

    let (result, _pool) = ori_types::check_module_with_imports(
        &parsed.module,
        &parsed.arena,
        &interner,
        |_checker| {},
    );

    assert!(
        !result.has_errors(),
        "Expected successful type check, but got errors:\n{}\nSource:\n{source}",
        format_typecheck_errors(result.errors())
    );

    result
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
            .any(|e| e.message().contains(expected_error));
        if has_expected {
            return;
        }
    }

    let (result, _pool) = ori_types::check_module_with_imports(
        &parsed.module,
        &parsed.arena,
        &interner,
        |_checker| {},
    );

    assert!(
        result.has_errors() || parsed.has_errors(),
        "Expected type check error containing '{expected_error}', but checking succeeded.\nSource: {source}"
    );

    let has_expected = result
        .errors()
        .iter()
        .any(|e| e.message().contains(expected_error));

    assert!(
        has_expected,
        "Expected type check error containing '{expected_error}', but got:\n{}\nSource:\n{source}",
        format_typecheck_errors(result.errors())
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
        let _ = write!(result, "Error {}: {}", i + 1, error.message());
    }
    result
}

#[cfg(test)]
mod tests;
