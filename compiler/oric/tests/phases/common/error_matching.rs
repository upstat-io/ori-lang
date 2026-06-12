//! Tests for `compile_fail` error matching (`oric::test::error_matching`).
//!
//! These tests verify:
//! - Message substring matching
//! - Error code matching
//! - Line/column matching
//! - Multi-criteria matching
//! - Batch error matching
//! - Unmatched expectation detection

use ori_types::{ErrorContext, Idx, Pool, TypeCheckError};
use oric::ir::{ExpectedError, SharedInterner, Span};
use oric::test::{format_actual, match_errors, matches_expected};

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
/// Produces message containing "unknown identifier". The name is interned so the
/// Pool-aware renderer can resolve it.
fn make_unknown_ident(offset: u32, interner: &SharedInterner) -> TypeCheckError {
    let name = interner.intern("missing_ident");
    TypeCheckError::unknown_ident(Span::new(offset, offset + 5), name, vec![])
}

#[test]
fn test_match_message() {
    let interner = SharedInterner::default();
    let pool = Pool::new();
    let source = "let x = 1\nlet y = 2";

    let err = make_mismatch(0);
    let exp = ExpectedError {
        message: Some(interner.intern("type mismatch")),
        code: None,
        line: None,
        column: None,
    };

    assert!(matches_expected(&err, &exp, source, &interner, &pool));
}

#[test]
fn test_match_code() {
    let interner = SharedInterner::default();
    let pool = Pool::new();
    let source = "let x = 1";

    let err = make_mismatch(0);
    let exp = ExpectedError {
        message: None,
        code: Some(interner.intern("E2001")),
        line: None,
        column: None,
    };

    assert!(matches_expected(&err, &exp, source, &interner, &pool));

    let exp_wrong = ExpectedError {
        message: None,
        code: Some(interner.intern("E2003")),
        line: None,
        column: None,
    };
    assert!(!matches_expected(
        &err, &exp_wrong, source, &interner, &pool
    ));
}

#[test]
fn test_match_line() {
    let interner = SharedInterner::default();
    let pool = Pool::new();
    let source = "line1\nline2\nline3";

    // Error at line 2 (offset 6 is start of "line2")
    let err = make_mismatch(6);
    let exp = ExpectedError {
        message: None,
        code: None,
        line: Some(2),
        column: None,
    };

    assert!(matches_expected(&err, &exp, source, &interner, &pool));

    let exp_wrong = ExpectedError {
        message: None,
        code: None,
        line: Some(1),
        column: None,
    };
    assert!(!matches_expected(
        &err, &exp_wrong, source, &interner, &pool
    ));
}

#[test]
fn test_match_multiple_criteria() {
    let interner = SharedInterner::default();
    let pool = Pool::new();
    let source = "line1\nline2";

    let err = make_mismatch(6);
    let exp = ExpectedError {
        message: Some(interner.intern("type mismatch")),
        code: Some(interner.intern("E2001")),
        line: Some(2),
        column: Some(1),
    };

    assert!(matches_expected(&err, &exp, source, &interner, &pool));
}

#[test]
fn test_match_errors_all_matched() {
    let interner = SharedInterner::default();
    let pool = Pool::new();
    let source = "line1\nline2";

    let errors = vec![make_mismatch(0), make_unknown_ident(6, &interner)];
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

    let result = match_errors(&errors, &expectations, source, &interner, &pool);
    assert!(result.all_matched());
    assert!(result.unmatched_expectations.is_empty());
}

#[test]
fn test_match_errors_unmatched_expectation() {
    let interner = SharedInterner::default();
    let pool = Pool::new();
    let source = "line1";

    let errors = vec![make_mismatch(0)];
    let expectations = vec![ExpectedError {
        message: Some(interner.intern("completely different")),
        code: None,
        line: None,
        column: None,
    }];

    let result = match_errors(&errors, &expectations, source, &interner, &pool);
    assert!(!result.all_matched());
    assert_eq!(result.unmatched_expectations.len(), 1);
}

/// Display pin: `format_actual` renders the real type name for a non-primitive
/// operand, not the `<type>` placeholder the Pool-less `message()` falls back to.
#[test]
fn format_actual_renders_real_type_name_for_non_primitive() {
    let interner = SharedInterner::default();
    let mut pool = Pool::new();
    let widget = interner.intern("Widget");
    let widget_ty = pool.named(widget);
    let source = "let r = a == b";

    let err = TypeCheckError::unsupported_operator(Span::new(8, 14), widget_ty, "==", "Eq");
    let rendered = format_actual(&err, source, &pool, &interner);

    assert!(
        rendered.contains("Widget"),
        "format_actual must name the real type; got: {rendered}"
    );
    assert!(
        !rendered.contains("<type>"),
        "format_actual must not emit the placeholder; got: {rendered}"
    );
}
