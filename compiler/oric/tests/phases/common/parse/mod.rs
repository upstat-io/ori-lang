//! Parse phase test utilities.
//!
//! Provides helpers for testing the lexer and parser.

use ori_ir::StringInterner;
use ori_lexer::lex;
use ori_parse::{parse, ParseError, ParseOutput};

/// Parse source code into a `ParseOutput`.
///
/// This is the primary entry point for parse phase tests. It handles
/// lexing and parsing in one step.
///
/// # Example
///
/// ```ignore
/// let output = parse_source("@add(a: int, b: int) -> int = a + b;");
/// assert!(!output.has_errors());
/// ```
pub fn parse_source(source: &str) -> ParseOutput {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    parse(&tokens, &interner)
}

/// Parse source code and assert it parses successfully.
///
/// Panics if there are any parse errors, printing the errors for debugging.
///
/// # Example
///
/// ```ignore
/// let output = parse_ok("@main () -> void = print(msg: \"hello\");");
/// assert!(output.module.functions.len() == 1);
/// ```
pub fn parse_ok(source: &str) -> ParseOutput {
    let output = parse_source(source);
    assert!(
        !output.has_errors(),
        "Expected successful parse, but got errors:\n{}",
        format_parse_errors(&output.errors, source)
    );
    output
}

/// Parse source code and assert it fails with an error containing the expected message.
///
/// Panics if parsing succeeds or if no error contains the expected substring.
///
/// # Example
///
/// ```ignore
/// parse_err("@foo( = 1;", "expected");
/// ```
pub fn parse_err(source: &str, expected_error: &str) {
    let output = parse_source(source);
    assert!(
        output.has_errors(),
        "Expected parse error containing '{expected_error}', but parsing succeeded.\nSource: {source}"
    );

    let has_expected = output
        .errors
        .iter()
        .any(|e| e.message().contains(expected_error));

    assert!(
        has_expected,
        "Expected parse error containing '{expected_error}', but got:\n{}",
        format_parse_errors(&output.errors, source)
    );
}

/// Format parse errors for display in test failure messages.
fn format_parse_errors(errors: &[ParseError], source: &str) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for (i, error) in errors.iter().enumerate() {
        if i > 0 {
            result.push_str("\n---\n");
        }
        let _ = writeln!(result, "Error {}: {error:?}", i + 1);
    }
    result.push_str("\nSource:\n");
    for (line_num, line) in source.lines().enumerate() {
        let _ = writeln!(result, "{:4} | {line}", line_num + 1);
    }
    result
}

#[cfg(test)]
mod tests;
