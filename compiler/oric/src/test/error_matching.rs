//! Error matching for `compile_fail` tests.
//!
//! Provides utilities for matching actual compilation errors against
//! expected error specifications.

use crate::ir::{ExpectedError, StringInterner};
use ori_diagnostic::span_utils;
use ori_ir::canon::PatternProblem;
use ori_types::TypeCheckError;

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
    match_errors_impl(actual.iter(), actual.len(), expected, source, interner)
}

/// Internal implementation for matching errors against expectations.
fn match_errors_impl<'a>(
    actual: impl Iterator<Item = &'a TypeCheckError>,
    actual_len: usize,
    expected: &[ExpectedError],
    source: &str,
    interner: &StringInterner,
) -> MatchResult {
    let actual: Vec<_> = actual.collect();
    let mut matched = Vec::new();
    let mut error_matched = vec![false; actual_len];

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
        if !actual.message().contains(msg_substr) {
            return false;
        }
    }

    // Check error code if specified
    if let Some(code_name) = expected.code {
        let code_str = interner.lookup(code_name);
        if actual.code().as_str() != code_str {
            return false;
        }
    }

    // Check line number if specified
    if let Some(line) = expected.line {
        let (actual_line, _) = span_utils::offset_to_line_col(source, actual.span().start);
        if actual_line != line {
            return false;
        }
    }

    // Check column number if specified
    if let Some(column) = expected.column {
        let (_, actual_col) = span_utils::offset_to_line_col(source, actual.span().start);
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
    let (line, col) = span_utils::offset_to_line_col(source, actual.span().start);
    format!(
        "[{}] at {}:{}: {}",
        actual.code().as_str(),
        line,
        col,
        actual.message()
    )
}

/// Check if a pattern problem matches an expected error specification.
///
/// Uses the same matching logic as [`matches_expected`] but adapted for
/// [`PatternProblem`] which carries different fields than [`TypeCheckError`].
pub fn matches_pattern_problem(
    problem: &PatternProblem,
    expected: &ExpectedError,
    source: &str,
    interner: &StringInterner,
) -> bool {
    let msg = format_pattern_problem_message(problem);

    // Check message substring if specified
    if let Some(msg_name) = expected.message {
        let msg_substr = interner.lookup(msg_name);
        if !msg.contains(msg_substr) {
            return false;
        }
    }

    // Check error code if specified
    if let Some(code_name) = expected.code {
        let code_str = interner.lookup(code_name);
        let actual_code = match problem {
            PatternProblem::NonExhaustive { .. } => "E3002",
            PatternProblem::RedundantArm { .. } => "E3003",
        };
        if actual_code != code_str {
            return false;
        }
    }

    // Check line/column against the primary span
    let span_start = match problem {
        PatternProblem::NonExhaustive { match_span, .. } => match_span.start,
        PatternProblem::RedundantArm { arm_span, .. } => arm_span.start,
    };

    if let Some(line) = expected.line {
        let (actual_line, _) = span_utils::offset_to_line_col(source, span_start);
        if actual_line != line {
            return false;
        }
    }

    if let Some(column) = expected.column {
        let (_, actual_col) = span_utils::offset_to_line_col(source, span_start);
        if actual_col != column {
            return false;
        }
    }

    true
}

/// Format a pattern problem's message for substring matching.
///
/// Produces the same format as `harness.rs` uses for eval errors:
/// - `"non-exhaustive match: missing patterns: X, Y"`
/// - `"redundant pattern: arm N is unreachable"`
fn format_pattern_problem_message(problem: &PatternProblem) -> String {
    match problem {
        PatternProblem::NonExhaustive { missing, .. } => {
            format!(
                "non-exhaustive match: missing patterns: {}",
                missing.join(", ")
            )
        }
        PatternProblem::RedundantArm { arm_index, .. } => {
            format!("redundant pattern: arm {arm_index} is unreachable")
        }
    }
}

/// Format a pattern problem for display in error messages.
pub fn format_pattern_problem(problem: &PatternProblem, source: &str) -> String {
    let (code, span_start) = match problem {
        PatternProblem::NonExhaustive { match_span, .. } => ("E3002", match_span.start),
        PatternProblem::RedundantArm { arm_span, .. } => ("E3003", arm_span.start),
    };
    let (line, col) = span_utils::offset_to_line_col(source, span_start);
    let msg = format_pattern_problem_message(problem);
    format!("[{code}] at {line}:{col}: {msg}")
}

/// Match both type errors and pattern problems against expected specifications.
///
/// Tries each expectation against type errors first, then pattern problems.
/// This unified approach handles `#compile_fail` tests that expect errors from
/// either the type checker or the exhaustiveness checker.
pub fn match_all_errors(
    type_errors: &[&TypeCheckError],
    pattern_problems: &[&PatternProblem],
    expected: &[ExpectedError],
    source: &str,
    interner: &StringInterner,
) -> MatchResult {
    let total_actual = type_errors.len() + pattern_problems.len();
    let mut matched = Vec::new();
    let mut type_error_matched = vec![false; type_errors.len()];
    let mut pattern_problem_matched = vec![false; pattern_problems.len()];

    // For each expectation, try type errors first, then pattern problems.
    for (exp_idx, exp) in expected.iter().enumerate() {
        let mut found = false;

        // Try type errors
        for (err_idx, err) in type_errors.iter().enumerate() {
            if !type_error_matched[err_idx] && matches_expected(err, exp, source, interner) {
                matched.push(exp_idx);
                type_error_matched[err_idx] = true;
                found = true;
                break;
            }
        }

        if found {
            continue;
        }

        // Try pattern problems
        for (pp_idx, pp) in pattern_problems.iter().enumerate() {
            if !pattern_problem_matched[pp_idx]
                && matches_pattern_problem(pp, exp, source, interner)
            {
                matched.push(exp_idx);
                pattern_problem_matched[pp_idx] = true;
                break;
            }
        }
    }

    let unmatched_expectations: Vec<usize> = (0..expected.len())
        .filter(|i| !matched.contains(i))
        .collect();

    let unmatched_errors: Vec<usize> = (0..total_actual)
        .filter(|i| {
            if *i < type_errors.len() {
                !type_error_matched[*i]
            } else {
                !pattern_problem_matched[*i - type_errors.len()]
            }
        })
        .collect();

    MatchResult {
        matched,
        unmatched_expectations,
        unmatched_errors,
    }
}
