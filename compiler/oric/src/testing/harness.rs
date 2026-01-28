//! Test harness utilities for compiler testing.
//!
//! This module provides utilities for testing the compiler components,
//! including expression evaluation helpers and test macros.

use crate::db::{CompilerDb, Db};
use crate::eval::{EvalError, EvalResult, Evaluator, Value};
use crate::ir::SharedInterner;
use crate::parser::{self, ParseOutput};
use crate::typeck::{type_check, TypedModule};

// Expression Evaluation Helpers

/// Evaluate a source expression and return the result value.
///
/// This is a convenience function for testing that handles all the
/// boilerplate of lexing, parsing, and evaluating.
///
/// # Example
///
/// ```text
/// use oric::testing::harness::eval_expr;
///
/// let result = eval_expr("1 + 2").unwrap();
/// assert_eq!(result, Value::int(3));
/// ```
pub fn eval_expr(source: &str) -> EvalResult {
    let db = CompilerDb::new();
    let interner = db.interner();
    let tokens = ori_lexer::lex(source, interner);
    let parsed = parser::parse(&tokens, interner);

    if parsed.has_errors() {
        return Err(EvalError::new(format!("parse errors: {:?}", parsed.errors)));
    }

    let mut evaluator = Evaluator::new(interner, &parsed.arena, &db);

    // Find and evaluate main function if it exists
    for func in &parsed.module.functions {
        let name = interner.lookup(func.name);
        if name == "main" {
            return evaluator.eval(func.body);
        }
    }

    // If no main function, try to evaluate as a single expression
    // This requires wrapping in a main function
    let wrapped_source = format!("@main () -> _ = {source}");
    eval_expr(&wrapped_source)
}

/// Evaluate a full source file with a main function.
pub fn eval_source(source: &str) -> EvalResult {
    let db = CompilerDb::new();
    let interner = db.interner();
    let tokens = ori_lexer::lex(source, interner);
    let parsed = parser::parse(&tokens, interner);

    if parsed.has_errors() {
        return Err(EvalError::new(format!("parse errors: {:?}", parsed.errors)));
    }

    let mut evaluator = Evaluator::new(interner, &parsed.arena, &db);

    // Find and evaluate main function
    for func in &parsed.module.functions {
        let name = interner.lookup(func.name);
        if name == "main" {
            return evaluator.eval(func.body);
        }
    }

    Err(EvalError::new("no main function found"))
}

// Parse Helpers

/// Parse source code and return the parse result.
pub fn parse_source(source: &str) -> (ParseOutput, SharedInterner) {
    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = parser::parse(&tokens, &interner);
    (parsed, interner)
}

/// Parse and type check source code.
pub fn type_check_source(source: &str) -> (ParseOutput, TypedModule, SharedInterner) {
    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = parser::parse(&tokens, &interner);
    let typed = type_check(&parsed, &interner);
    (parsed, typed, interner)
}

// Assertion Helpers

/// Assert that evaluation produces the expected integer value.
pub fn assert_eval_int(source: &str, expected: i64) {
    let wrapped = format!("@main () -> int = {source}");
    match eval_source(&wrapped) {
        Ok(Value::Int(n)) => assert_eq!(n.raw(), expected, "source: {source}"),
        Ok(other) => panic!("expected Int({expected}), got {other:?} for: {source}"),
        Err(e) => panic!("evaluation error for '{source}': {e:?}"),
    }
}

/// Assert that evaluation produces the expected float value.
pub fn assert_eval_float(source: &str, expected: f64) {
    let wrapped = format!("@main () -> float = {source}");
    match eval_source(&wrapped) {
        Ok(Value::Float(f)) => {
            assert!(
                (f - expected).abs() < 1e-10,
                "expected Float({expected}), got Float({f}) for: {source}"
            );
        }
        Ok(other) => panic!("expected Float({expected}), got {other:?} for: {source}"),
        Err(e) => panic!("evaluation error for '{source}': {e:?}"),
    }
}

/// Assert that evaluation produces the expected boolean value.
pub fn assert_eval_bool(source: &str, expected: bool) {
    let wrapped = format!("@main () -> bool = {source}");
    match eval_source(&wrapped) {
        Ok(Value::Bool(b)) => assert_eq!(b, expected, "source: {source}"),
        Ok(other) => panic!("expected Bool({expected}), got {other:?} for: {source}"),
        Err(e) => panic!("evaluation error for '{source}': {e:?}"),
    }
}

/// Assert that evaluation produces the expected string value.
pub fn assert_eval_str(source: &str, expected: &str) {
    let wrapped = format!("@main () -> str = {source}");
    match eval_source(&wrapped) {
        Ok(Value::Str(s)) => assert_eq!(s.as_str(), expected, "source: {source}"),
        Ok(other) => panic!("expected Str({expected}), got {other:?} for: {source}"),
        Err(e) => panic!("evaluation error for '{source}': {e:?}"),
    }
}

/// Assert that parsing fails with an error.
pub fn assert_parse_error(source: &str) {
    let (parsed, _interner) = parse_source(source);
    assert!(
        parsed.has_errors(),
        "expected parse error but parsing succeeded for: {source}"
    );
}

/// Assert that type checking produces an error.
pub fn assert_type_error(source: &str) {
    let (_, typed, _interner) = type_check_source(source);
    assert!(
        typed.has_errors(),
        "expected type error but type checking succeeded for: {source}"
    );
}

/// Assert that evaluation produces an error.
pub fn assert_eval_error(source: &str) {
    let wrapped = format!("@main () -> _ = {source}");
    if let Ok(v) = eval_source(&wrapped) {
        panic!("expected error but got {v:?} for: {source}");
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_eval_source() {
        let result = eval_source("@main () -> int = 42");
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn test_assert_eval_int() {
        assert_eval_int("1 + 2", 3);
        assert_eval_int("10 - 3", 7);
        assert_eval_int("4 * 5", 20);
    }

    #[test]
    fn test_assert_eval_bool() {
        assert_eval_bool("true", true);
        assert_eval_bool("false", false);
        assert_eval_bool("1 == 1", true);
        assert_eval_bool("1 == 2", false);
    }

    #[test]
    fn test_assert_eval_str() {
        assert_eval_str("\"hello\"", "hello");
        assert_eval_str("\"a\" + \"b\"", "ab");
    }

    #[test]
    fn test_parse_source() {
        let (parsed, _interner) = parse_source("@main () -> int = 42");
        assert!(!parsed.has_errors());
        assert_eq!(parsed.module.functions.len(), 1);
    }

    #[test]
    fn test_type_check_source() {
        let (_, typed, _interner) = type_check_source("@main () -> int = 42");
        assert!(!typed.has_errors());
    }
}
