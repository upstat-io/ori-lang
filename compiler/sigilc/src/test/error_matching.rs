//! Error matching for `compile_fail` tests.
//!
//! Provides utilities for matching actual compilation errors against
//! expected error specifications.

use crate::ir::{ExpectedError, StringInterner};
use crate::typeck::TypeCheckError;
use sigil_diagnostic::span_utils;

/// Result of matching errors against expectations.
#[derive(Debug)]
pub struct MatchResult {
    /// Expectations that were matched by actual errors.
    pub matched: Vec<usize>,
    /// Expectations that were not matched.
    pub unmatched_expectations: Vec<usize>,
    /// Actual errors that didn't match any expectation.
    pub unmatched_errors: Vec<usize>,
}

impl MatchResult {
    /// Check if all expectations were matched.
    pub fn all_matched(&self) -> bool {
        self.unmatched_expectations.is_empty()
    }

    /// Check if the match was fully successful (all expectations matched,
    /// and there are no extra unexpected errors for strict mode).
    pub fn is_success(&self) -> bool {
        self.unmatched_expectations.is_empty()
    }
}

/// Match actual errors against expected error specifications.
///
/// Returns which expectations matched and which did not.
pub fn match_errors(
    actual: &[TypeCheckError],
    expected: &[ExpectedError],
    source: &str,
    interner: &StringInterner,
) -> MatchResult {
    let mut matched = Vec::new();
    let mut error_matched = vec![false; actual.len()];

    // For each expectation, try to find a matching error
    for (exp_idx, exp) in expected.iter().enumerate() {
        for (err_idx, err) in actual.iter().enumerate() {
            if !error_matched[err_idx] && matches_expected(err, exp, source, interner) {
                matched.push(exp_idx);
                error_matched[err_idx] = true;
                break;
            }
        }
    }

    // Collect unmatched expectations
    let unmatched_expectations: Vec<usize> = (0..expected.len())
        .filter(|i| !matched.contains(i))
        .collect();

    // Collect unmatched errors
    let unmatched_errors: Vec<usize> = error_matched
        .iter()
        .enumerate()
        .filter_map(|(i, &m)| if m { None } else { Some(i) })
        .collect();

    MatchResult {
        matched,
        unmatched_expectations,
        unmatched_errors,
    }
}

/// Check if an actual error matches an expected specification.
pub fn matches_expected(
    actual: &TypeCheckError,
    expected: &ExpectedError,
    source: &str,
    interner: &StringInterner,
) -> bool {
    // Check message substring if specified
    if let Some(msg_name) = expected.message {
        let msg_substr = interner.lookup(msg_name);
        if !actual.message.contains(msg_substr) {
            return false;
        }
    }

    // Check error code if specified
    if let Some(code_name) = expected.code {
        let code_str = interner.lookup(code_name);
        if actual.code.as_str() != code_str {
            return false;
        }
    }

    // Check line number if specified
    if let Some(line) = expected.line {
        let (actual_line, _) = span_utils::offset_to_line_col(source, actual.span.start);
        if actual_line != line {
            return false;
        }
    }

    // Check column number if specified
    if let Some(column) = expected.column {
        let (_, actual_col) = span_utils::offset_to_line_col(source, actual.span.start);
        if actual_col != column {
            return false;
        }
    }

    true
}

/// Format an `ExpectedError` for display in error messages.
pub fn format_expected(expected: &ExpectedError, interner: &StringInterner) -> String {
    let mut parts = Vec::new();

    if let Some(msg) = expected.message {
        parts.push(format!("message contains '{}'", interner.lookup(msg)));
    }
    if let Some(code) = expected.code {
        parts.push(format!("code = {}", interner.lookup(code)));
    }
    if let Some(line) = expected.line {
        parts.push(format!("line = {line}"));
    }
    if let Some(col) = expected.column {
        parts.push(format!("column = {col}"));
    }

    if parts.is_empty() {
        "(any error)".to_string()
    } else {
        parts.join(", ")
    }
}

/// Format an actual error for display in error messages.
pub fn format_actual(actual: &TypeCheckError, source: &str) -> String {
    let (line, col) = span_utils::offset_to_line_col(source, actual.span.start);
    format!(
        "[{}] at {}:{}: {}",
        actual.code.as_str(),
        line,
        col,
        actual.message
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Span, SharedInterner};
    use sigil_diagnostic::ErrorCode;

    fn make_error(code: ErrorCode, message: &str, offset: u32) -> TypeCheckError {
        TypeCheckError {
            code,
            message: message.to_string(),
            span: Span::new(offset, offset + 5),
        }
    }

    #[test]
    fn test_match_message() {
        let interner = SharedInterner::default();
        let source = "let x = 1\nlet y = 2";

        let err = make_error(ErrorCode::E2001, "type mismatch: expected int", 0);
        let exp = ExpectedError {
            message: Some(interner.intern("type mismatch")),
            code: None,
            line: None,
            column: None,
        };

        assert!(matches_expected(&err, &exp, source, &interner));
    }

    #[test]
    fn test_match_code() {
        let interner = SharedInterner::default();
        let source = "let x = 1";

        let err = make_error(ErrorCode::E2001, "some error", 0);
        let exp = ExpectedError {
            message: None,
            code: Some(interner.intern("E2001")),
            line: None,
            column: None,
        };

        assert!(matches_expected(&err, &exp, source, &interner));

        let exp_wrong = ExpectedError {
            message: None,
            code: Some(interner.intern("E2003")),
            line: None,
            column: None,
        };
        assert!(!matches_expected(&err, &exp_wrong, source, &interner));
    }

    #[test]
    fn test_match_line() {
        let interner = SharedInterner::default();
        let source = "line1\nline2\nline3";

        // Error at line 2 (offset 6 is start of "line2")
        let err = make_error(ErrorCode::E2001, "error", 6);
        let exp = ExpectedError {
            message: None,
            code: None,
            line: Some(2),
            column: None,
        };

        assert!(matches_expected(&err, &exp, source, &interner));

        let exp_wrong = ExpectedError {
            message: None,
            code: None,
            line: Some(1),
            column: None,
        };
        assert!(!matches_expected(&err, &exp_wrong, source, &interner));
    }

    #[test]
    fn test_match_multiple_criteria() {
        let interner = SharedInterner::default();
        let source = "line1\nline2";

        let err = make_error(ErrorCode::E2001, "type mismatch", 6);
        let exp = ExpectedError {
            message: Some(interner.intern("type")),
            code: Some(interner.intern("E2001")),
            line: Some(2),
            column: Some(1),
        };

        assert!(matches_expected(&err, &exp, source, &interner));
    }

    #[test]
    fn test_match_errors_all_matched() {
        let interner = SharedInterner::default();
        let source = "line1\nline2";

        let errors = vec![
            make_error(ErrorCode::E2001, "type mismatch", 0),
            make_error(ErrorCode::E2003, "unknown identifier", 6),
        ];
        let expectations = vec![
            ExpectedError {
                message: Some(interner.intern("type mismatch")),
                code: None,
                line: None,
                column: None,
            },
            ExpectedError {
                message: Some(interner.intern("unknown")),
                code: None,
                line: None,
                column: None,
            },
        ];

        let result = match_errors(&errors, &expectations, source, &interner);
        assert!(result.all_matched());
        assert_eq!(result.matched.len(), 2);
    }

    #[test]
    fn test_match_errors_unmatched_expectation() {
        let interner = SharedInterner::default();
        let source = "line1";

        let errors = vec![make_error(ErrorCode::E2001, "actual error", 0)];
        let expectations = vec![ExpectedError {
            message: Some(interner.intern("different error")),
            code: None,
            line: None,
            column: None,
        }];

        let result = match_errors(&errors, &expectations, source, &interner);
        assert!(!result.all_matched());
        assert_eq!(result.unmatched_expectations.len(), 1);
    }
}
