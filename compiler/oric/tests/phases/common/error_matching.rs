//! Tests for `compile_fail` error matching (`oric::test::error_matching`).
//!
//! These tests verify:
//! - Message substring matching
//! - Error code matching
//! - Line/column matching
//! - Multi-criteria matching
//! - Batch error matching
//! - Unmatched expectation detection

use ori_diagnostic::ErrorCode;
use oric::ir::{ExpectedError, SharedInterner, Span};
use oric::test::{match_errors, matches_expected, MatchResult};
use oric::TypeCheckError;

fn make_error(code: ErrorCode, message: &str, offset: u32) -> TypeCheckError {
    TypeCheckError::Generic {
        code,
        message: message.to_string(),
        span: Span::new(offset, offset + 5),
        suggestion: None,
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
