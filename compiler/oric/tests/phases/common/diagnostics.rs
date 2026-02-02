//! Tests for diagnostic queue (`ori_diagnostic::queue`).
//!
//! These tests verify:
//! - Basic queue operations (add, flush)
//! - Error limits
//! - Soft error suppression after hard errors
//! - Follow-on error filtering
//! - Deduplication (syntax and semantic)
//! - Sorting by position
//! - `ErrorGuaranteed` proof type

use ori_diagnostic::queue::{too_many_errors, DiagnosticConfig, DiagnosticQueue};
use ori_diagnostic::{Diagnostic, DiagnosticSeverity, ErrorCode};
use ori_ir::Span;

fn make_error(code: ErrorCode, message: &str, start: u32) -> Diagnostic {
    Diagnostic::error(code)
        .with_message(message)
        .with_label(Span::new(start, start + 5), "error here")
}

#[test]
fn test_queue_basic() {
    let mut queue = DiagnosticQueue::new();

    let diag = make_error(ErrorCode::E2001, "type mismatch", 0);
    assert!(queue.add_with_severity(diag, 1, 1, DiagnosticSeverity::Hard));
    assert_eq!(queue.error_count(), 1);

    let errors = queue.flush();
    assert_eq!(errors.len(), 1);
    assert_eq!(queue.error_count(), 0);
}

#[test]
fn test_queue_error_limit() {
    let config = DiagnosticConfig {
        error_limit: 2,
        ..Default::default()
    };
    let mut queue = DiagnosticQueue::with_config(config);

    // Add up to limit
    assert!(queue.add_with_severity(
        make_error(ErrorCode::E2001, "error 1", 0),
        1,
        1,
        DiagnosticSeverity::Hard
    ));
    assert!(queue.add_with_severity(
        make_error(ErrorCode::E2001, "error 2", 10),
        2,
        1,
        DiagnosticSeverity::Hard
    ));

    // At limit
    assert!(queue.limit_reached());

    // Rejected
    assert!(!queue.add_with_severity(
        make_error(ErrorCode::E2001, "error 3", 20),
        3,
        1,
        DiagnosticSeverity::Hard
    ));

    let errors = queue.flush();
    assert_eq!(errors.len(), 2);
}

#[test]
fn test_queue_soft_error_suppression() {
    let mut queue = DiagnosticQueue::new();

    // Hard error
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "hard error", 0),
        1,
        1,
        DiagnosticSeverity::Hard,
    );
    assert!(queue.has_hard_error());

    // Soft error should be suppressed
    assert!(!queue.add_with_severity(
        make_error(ErrorCode::E2005, "soft error", 10),
        2,
        1,
        DiagnosticSeverity::Soft
    ));

    let errors = queue.flush();
    assert_eq!(errors.len(), 1);
}

#[test]
fn test_queue_follow_on_filtering() {
    let mut queue = DiagnosticQueue::new();

    // First error
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "type mismatch", 0),
        1,
        1,
        DiagnosticSeverity::Hard,
    );

    // Follow-on error should be filtered
    assert!(!queue.add_with_severity(
        make_error(
            ErrorCode::E2001,
            "invalid operand due to previous error",
            10
        ),
        2,
        1,
        DiagnosticSeverity::Hard
    ));

    let errors = queue.flush();
    assert_eq!(errors.len(), 1);
}

#[test]
fn test_queue_deduplication_syntax() {
    let mut queue = DiagnosticQueue::new();

    // First syntax error on line 1
    queue.add_with_severity(
        make_error(ErrorCode::E1001, "unexpected token", 0),
        1,
        1,
        DiagnosticSeverity::Hard,
    );

    // Second syntax error on same line should be deduped
    assert!(!queue.add_with_severity(
        make_error(ErrorCode::E1002, "expected expression", 5),
        1,
        5,
        DiagnosticSeverity::Hard
    ));

    // Error on different line should be added
    assert!(queue.add_with_severity(
        make_error(ErrorCode::E1001, "another error", 20),
        2,
        1,
        DiagnosticSeverity::Hard
    ));

    let errors = queue.flush();
    assert_eq!(errors.len(), 2);
}

#[test]
fn test_queue_deduplication_same_message() {
    let mut queue = DiagnosticQueue::new();

    // Same message on same line
    queue.add_with_severity(
        make_error(
            ErrorCode::E2001,
            "type mismatch: expected int, found str",
            0,
        ),
        1,
        1,
        DiagnosticSeverity::Hard,
    );
    assert!(!queue.add_with_severity(
        make_error(
            ErrorCode::E2001,
            "type mismatch: expected int, found str",
            5
        ),
        1,
        5,
        DiagnosticSeverity::Hard
    ));

    // Different message on same line should be added
    assert!(queue.add_with_severity(
        make_error(ErrorCode::E2001, "different error message here", 10),
        1,
        10,
        DiagnosticSeverity::Hard
    ));

    let errors = queue.flush();
    assert_eq!(errors.len(), 2);
}

#[test]
fn test_queue_sorting() {
    let mut queue = DiagnosticQueue::with_config(DiagnosticConfig::unlimited());

    // Add out of order
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "error on line 3", 40),
        3,
        1,
        DiagnosticSeverity::Hard,
    );
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "error on line 1", 0),
        1,
        1,
        DiagnosticSeverity::Hard,
    );
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "error on line 2", 20),
        2,
        1,
        DiagnosticSeverity::Hard,
    );

    let errors = queue.flush();
    assert_eq!(errors.len(), 3);
    assert!(errors[0].message.contains("line 1"));
    assert!(errors[1].message.contains("line 2"));
    assert!(errors[2].message.contains("line 3"));
}

#[test]
fn test_queue_sorting_within_line() {
    let mut queue = DiagnosticQueue::with_config(DiagnosticConfig::unlimited());

    // Add errors on same line, out of order by column
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "error at col 10", 10),
        1,
        10,
        DiagnosticSeverity::Hard,
    );
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "error at col 1", 0),
        1,
        1,
        DiagnosticSeverity::Hard,
    );
    queue.add_with_severity(
        make_error(ErrorCode::E2001, "error at col 5", 5),
        1,
        5,
        DiagnosticSeverity::Hard,
    );

    let errors = queue.flush();
    assert_eq!(errors.len(), 3);
    assert!(errors[0].message.contains("col 1"));
    assert!(errors[1].message.contains("col 5"));
    assert!(errors[2].message.contains("col 10"));
}

#[test]
fn test_too_many_errors_diagnostic() {
    let diag = too_many_errors(10, Span::new(0, 5));
    assert_eq!(diag.code, ErrorCode::E9002);
    assert!(diag.message.contains("10"));
}

#[test]
fn test_warnings_not_counted() {
    let config = DiagnosticConfig {
        error_limit: 1,
        ..Default::default()
    };
    let mut queue = DiagnosticQueue::with_config(config);

    // Warning doesn't count toward limit
    let warning = Diagnostic::warning(ErrorCode::E2001)
        .with_message("warning")
        .with_label(Span::new(0, 5), "here");
    assert!(queue.add_with_severity(warning, 1, 1, DiagnosticSeverity::Hard));
    assert!(!queue.limit_reached());
    assert_eq!(queue.error_count(), 0);

    // Error counts
    assert!(queue.add_with_severity(
        make_error(ErrorCode::E2001, "error", 10),
        2,
        1,
        DiagnosticSeverity::Hard
    ));
    assert!(queue.limit_reached());
}

#[test]
fn test_emit_error_returns_guarantee() {
    let mut queue = DiagnosticQueue::new();

    // No errors yet
    assert!(queue.has_errors().is_none());

    // emit_error returns a guarantee
    let diag = make_error(ErrorCode::E2001, "test error", 0);
    let _guarantee = queue.emit_error(diag, 1, 1);

    // Now has_errors returns Some
    assert!(queue.has_errors().is_some());
    assert_eq!(queue.error_count(), 1);
}

#[test]
fn test_emit_error_with_source() {
    let mut queue = DiagnosticQueue::new();
    let source = "let x = 42\nlet y = true";

    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("test")
        .with_label(Span::new(11, 15), "here"); // Line 2

    let _guarantee = queue.emit_error_with_source(diag, source);

    assert!(queue.has_errors().is_some());
}

#[test]
fn test_error_guaranteed_salsa_traits() {
    use std::collections::HashSet;

    let mut queue = DiagnosticQueue::new();
    let diag = make_error(ErrorCode::E2001, "error", 0);
    let g1 = queue.emit_error(diag.clone(), 1, 1);
    let g2 = queue.emit_error(diag, 2, 1);

    // ErrorGuaranteed implements Eq
    assert_eq!(g1, g2);

    // ErrorGuaranteed implements Hash
    let mut set = HashSet::new();
    set.insert(g1);
    set.insert(g2); // duplicate
    assert_eq!(set.len(), 1);
}
