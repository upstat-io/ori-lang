use super::*;
use std::collections::HashSet;

#[test]
fn test_diagnostic_severity_hash() {
    let mut set = HashSet::new();
    set.insert(DiagnosticSeverity::Hard);
    set.insert(DiagnosticSeverity::Soft);
    assert_eq!(set.len(), 2);
    assert!(set.contains(&DiagnosticSeverity::Hard));
    assert!(set.contains(&DiagnosticSeverity::Soft));
}

#[test]
fn test_soft_errors_suppressed_after_hard() {
    let source = "let x = 1\nlet y = 2\nlet z = 3\n";
    let mut queue = DiagnosticQueue::new();

    // Add a hard error
    let hard_diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("hard error")
        .with_label(Span::new(0, 5), "here");
    assert!(queue.add_with_source_and_severity(hard_diag, source, DiagnosticSeverity::Hard,));

    // Add a soft error — should be suppressed
    let soft_diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("soft error")
        .with_label(Span::new(20, 5), "here");
    assert!(!queue.add_with_source_and_severity(soft_diag, source, DiagnosticSeverity::Soft,));

    let flushed = queue.flush();
    assert_eq!(flushed.len(), 1);
    assert_eq!(flushed[0].message, "hard error");
}

#[test]
fn test_soft_errors_reported_when_no_hard_error() {
    let source = "let x = 1\nlet y = 2\n";
    let mut queue = DiagnosticQueue::new();

    // Add only soft errors
    let soft1 = Diagnostic::error(ErrorCode::E1001)
        .with_message("soft error 1")
        .with_label(Span::new(0, 5), "here");
    assert!(queue.add_with_source_and_severity(soft1, source, DiagnosticSeverity::Soft,));

    let soft2 = Diagnostic::error(ErrorCode::E1001)
        .with_message("soft error 2")
        .with_label(Span::new(10, 5), "here");
    assert!(queue.add_with_source_and_severity(soft2, source, DiagnosticSeverity::Soft,));

    let flushed = queue.flush();
    assert_eq!(flushed.len(), 2);
}

#[test]
fn test_hard_errors_not_suppressed() {
    let source = "let x = 1\nlet y = 2\n";
    let mut queue = DiagnosticQueue::new();

    // Add two hard errors
    let hard1 = Diagnostic::error(ErrorCode::E1001)
        .with_message("first hard")
        .with_label(Span::new(0, 5), "here");
    assert!(queue.add_with_source_and_severity(hard1, source, DiagnosticSeverity::Hard,));

    let hard2 = Diagnostic::error(ErrorCode::E1002)
        .with_message("second hard")
        .with_label(Span::new(10, 5), "here");
    assert!(queue.add_with_source_and_severity(hard2, source, DiagnosticSeverity::Hard,));

    let flushed = queue.flush();
    assert_eq!(flushed.len(), 2);
}
