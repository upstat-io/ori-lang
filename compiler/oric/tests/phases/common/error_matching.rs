//! Tests for `compile_fail` error matching (`oric::test::error_matching`).
//!
//! These tests verify:
//! - Message substring matching
//! - Error code matching
//! - Line/column matching
//! - Multi-criteria matching
//! - Batch error matching
//! - Unmatched expectation detection

use ori_ir::Name;
use ori_types::{ErrorContext, Idx, TypeCheckError};
use oric::ir::{ExpectedError, SharedInterner, Span};
use oric::test::{match_errors, matches_expected, MatchResult};

/// Create a mismatch error (E2001) at the given offset.
///
/// Produces message like "expected int, found float".
fn make_mismatch(offset: u32) -> TypeCheckError {
    TypeCheckError::mismatch(
        Span::new(offset, offset + 5),
        Idx::INT,
        Idx::FLOAT,
        vec![],
        ErrorContext::default(),
    )
}

/// Create an unknown identifier error (E2003) at the given offset.
///
/// Produces message containing "unknown identifier".
fn make_unknown_ident(offset: u32) -> TypeCheckError {
    TypeCheckError::unknown_ident(Span::new(offset, offset + 5), Name::from_raw(999), vec![])
}

#[test]
fn test_match_message() {
    let interner = SharedInterner::default();
    let source = "let x = 1\nlet y = 2";

    let err = make_mismatch(0);
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

    let err = make_mismatch(0);
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
    let err = make_mismatch(6);
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

    let err = make_mismatch(6);
    let exp = ExpectedError {
        message: Some(interner.intern("type mismatch")),
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

    let errors = vec![make_mismatch(0), make_unknown_ident(6)];
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

    let errors = vec![make_mismatch(0)];
    let expectations = vec![ExpectedError {
        message: Some(interner.intern("completely different")),
        code: None,
        line: None,
        column: None,
    }];

    let result = match_errors(&errors, &expectations, source, &interner);
    assert!(!result.all_matched());
    assert_eq!(result.unmatched_expectations.len(), 1);
}
