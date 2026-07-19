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

// Deduplication: consecutive syntax (parser/E1xxx) errors on the same line.

#[test]
fn test_syntax_errors_same_line_deduplicated() {
    let mut queue = DiagnosticQueue::new();

    let first = Diagnostic::error(ErrorCode::E1001)
        .with_message("unexpected token")
        .with_label(Span::new(0, 1), "here");
    assert!(queue.add_with_severity(first, 1, 1, DiagnosticSeverity::Hard));

    // Different message, SAME line, also a parser error -> suppressed by syntax dedup.
    let second = Diagnostic::error(ErrorCode::E1002)
        .with_message("expected expression")
        .with_label(Span::new(2, 3), "here");
    assert!(!queue.add_with_severity(second, 1, 3, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 1);
}

#[test]
fn test_syntax_errors_different_lines_not_deduplicated() {
    let mut queue = DiagnosticQueue::new();

    let first = Diagnostic::error(ErrorCode::E1001)
        .with_message("unexpected token")
        .with_label(Span::new(0, 1), "here");
    assert!(queue.add_with_severity(first, 1, 1, DiagnosticSeverity::Hard));

    // Same parser-error class but a DIFFERENT line -> kept.
    let second = Diagnostic::error(ErrorCode::E1001)
        .with_message("unexpected token")
        .with_label(Span::new(10, 11), "here");
    assert!(queue.add_with_severity(second, 2, 1, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 2);
}

// Deduplication: non-syntax (E2xxx) errors on the same line with the same message prefix.

#[test]
fn test_non_syntax_errors_same_line_same_prefix_deduplicated() {
    let mut queue = DiagnosticQueue::new();

    let first = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch: expected `int`, found `str`")
        .with_label(Span::new(0, 5), "here");
    assert!(queue.add_with_severity(first, 1, 1, DiagnosticSeverity::Hard));

    // Same line, same 30-char message prefix -> suppressed by non-syntax dedup.
    let second = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch: expected `int`, found `bool`")
        .with_label(Span::new(6, 9), "here");
    assert!(!queue.add_with_severity(second, 1, 7, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 1);
}

#[test]
fn test_non_syntax_errors_same_line_different_prefix_not_deduplicated() {
    let mut queue = DiagnosticQueue::new();

    let first = Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch: expected `int`, found `str`")
        .with_label(Span::new(0, 5), "here");
    assert!(queue.add_with_severity(first, 1, 1, DiagnosticSeverity::Hard));

    // Same line, but a clearly different message prefix -> kept.
    let second = Diagnostic::error(ErrorCode::E2009)
        .with_message("missing trait bound `Eq` for this type parameter")
        .with_label(Span::new(6, 9), "here");
    assert!(queue.add_with_severity(second, 1, 7, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 2);
}

#[test]
fn test_dedup_disabled_keeps_duplicates() {
    let mut queue = DiagnosticQueue::with_config(DiagnosticConfig::unlimited());

    let first = Diagnostic::error(ErrorCode::E1001)
        .with_message("unexpected token")
        .with_label(Span::new(0, 1), "here");
    assert!(queue.add_with_severity(first, 1, 1, DiagnosticSeverity::Hard));

    // Same line, same parser class, but dedup is off in `unlimited()` -> kept.
    let second = Diagnostic::error(ErrorCode::E1001)
        .with_message("unexpected token")
        .with_label(Span::new(2, 3), "here");
    assert!(queue.add_with_severity(second, 1, 3, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 2);
}

// Follow-on filtering: cascading errors with marker phrases are dropped.

#[test]
fn test_follow_on_error_filtered() {
    let mut queue = DiagnosticQueue::new();

    let follow_on = Diagnostic::error(ErrorCode::E2011)
        .with_message("invalid operand for this expression")
        .with_label(Span::new(0, 5), "here");
    // filter_follow_on is on by default -> "invalid operand" is suppressed.
    assert!(!queue.add_with_severity(follow_on, 1, 1, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 0);
}

#[test]
fn test_follow_on_error_kept_when_filter_disabled() {
    let config = DiagnosticConfig {
        error_limit: 0,
        filter_follow_on: false,
        deduplicate: false,
    };
    let mut queue = DiagnosticQueue::with_config(config);

    let follow_on = Diagnostic::error(ErrorCode::E2011)
        .with_message("invalid operand for this expression")
        .with_label(Span::new(0, 5), "here");
    // Filter is off -> the same diagnostic is kept.
    assert!(queue.add_with_severity(follow_on, 1, 1, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 1);
}

#[test]
fn test_non_follow_on_error_not_filtered() {
    let mut queue = DiagnosticQueue::new();

    // A normal error with no follow-on marker phrase -> kept even with filter on.
    let normal = Diagnostic::error(ErrorCode::E2003)
        .with_message("unknown identifier `foo`")
        .with_label(Span::new(0, 3), "here");
    assert!(queue.add_with_severity(normal, 1, 1, DiagnosticSeverity::Hard));

    assert_eq!(queue.flush().len(), 1);
}

// Error limit: diagnostics beyond the configured cap are dropped.

#[test]
fn test_error_limit_caps_diagnostics() {
    let config = DiagnosticConfig {
        error_limit: 2,
        filter_follow_on: false,
        deduplicate: false,
    };
    let mut queue = DiagnosticQueue::with_config(config);

    // Two errors fit under the limit (distinct lines avoid dedup paths).
    let e1 = Diagnostic::error(ErrorCode::E2003).with_message("err 1");
    assert!(queue.add_with_severity(e1, 1, 1, DiagnosticSeverity::Hard));
    assert!(!queue.limit_reached());

    let e2 = Diagnostic::error(ErrorCode::E2003).with_message("err 2");
    assert!(queue.add_with_severity(e2, 2, 1, DiagnosticSeverity::Hard));
    assert!(queue.limit_reached());

    // The third is rejected because the limit was reached.
    let e3 = Diagnostic::error(ErrorCode::E2003).with_message("err 3");
    assert!(!queue.add_with_severity(e3, 3, 1, DiagnosticSeverity::Hard));

    assert_eq!(queue.error_count(), 2);
    assert_eq!(queue.flush().len(), 2);
}

#[test]
fn test_error_limit_zero_is_unlimited() {
    let config = DiagnosticConfig {
        error_limit: 0,
        filter_follow_on: false,
        deduplicate: false,
    };
    let mut queue = DiagnosticQueue::with_config(config);

    for i in 0..50u32 {
        let diag = Diagnostic::error(ErrorCode::E2003).with_message(format!("err {i}"));
        assert!(queue.add_with_severity(diag, i + 1, 1, DiagnosticSeverity::Hard));
    }
    assert!(!queue.limit_reached());
    assert_eq!(queue.flush().len(), 50);
}

// ErrorGuaranteed emission paths.

#[test]
fn test_emit_error_records_and_proves() {
    let mut queue = DiagnosticQueue::new();
    assert!(queue.has_errors().is_none());

    let diag = Diagnostic::error(ErrorCode::E2003).with_message("unknown identifier");
    // emit_error returns ErrorGuaranteed by value (type-level proof) — calling it suffices.
    let _proof = queue.emit_error(diag, 1, 1);

    assert_eq!(queue.error_count(), 1);
    assert!(queue.has_errors().is_some());
    assert!(queue.has_hard_error());
}

#[test]
fn test_emit_error_with_source_computes_position() {
    let mut queue = DiagnosticQueue::new();
    let source = "let x = 1\nlet y = bad";

    let diag = Diagnostic::error(ErrorCode::E2003)
        .with_message("unknown identifier `bad`")
        .with_label(Span::new(18, 21), "not found");
    let _proof = queue.emit_error_with_source(diag, source);

    let flushed = queue.flush();
    assert_eq!(flushed.len(), 1);
    assert_eq!(flushed[0].message, "unknown identifier `bad`");
}

#[test]
fn test_warning_does_not_set_hard_error_or_count() {
    let mut queue = DiagnosticQueue::new();

    let warn = Diagnostic::warning(ErrorCode::W2001)
        .with_message("infinite iterator consumed without bound")
        .with_label(Span::new(0, 5), "here");
    assert!(queue.add_with_severity(warn, 1, 1, DiagnosticSeverity::Hard));

    // Warnings are not errors: no hard-error flag, no error count, no ErrorGuaranteed.
    assert!(!queue.has_hard_error());
    assert_eq!(queue.error_count(), 0);
    assert!(queue.has_errors().is_none());
    assert_eq!(queue.flush().len(), 1);
}

// flush() sorts by (line, column) and resets queue state.

#[test]
fn test_flush_sorts_by_position() {
    let config = DiagnosticConfig {
        error_limit: 0,
        filter_follow_on: false,
        deduplicate: false,
    };
    let mut queue = DiagnosticQueue::with_config(config);

    // Add out of order: (line 3), (line 1, col 5), (line 1, col 1).
    queue.add_with_severity(
        Diagnostic::error(ErrorCode::E2003).with_message("third"),
        3,
        1,
        DiagnosticSeverity::Hard,
    );
    queue.add_with_severity(
        Diagnostic::error(ErrorCode::E2003).with_message("second"),
        1,
        5,
        DiagnosticSeverity::Hard,
    );
    queue.add_with_severity(
        Diagnostic::error(ErrorCode::E2003).with_message("first"),
        1,
        1,
        DiagnosticSeverity::Hard,
    );

    let flushed = queue.flush();
    let messages: Vec<&str> = flushed.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, vec!["first", "second", "third"]);
}

#[test]
fn test_flush_resets_state() {
    let mut queue = DiagnosticQueue::new();

    let diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("first")
        .with_label(Span::new(0, 1), "here");
    queue.add_with_severity(diag, 1, 1, DiagnosticSeverity::Hard);
    assert_eq!(queue.flush().len(), 1);

    // After flush the queue is empty and counters/dedup state are reset.
    assert_eq!(queue.error_count(), 0);
    assert!(!queue.has_hard_error());
    assert!(queue.has_errors().is_none());

    // A previously-deduplicated same-line syntax error is now accepted again.
    let again = Diagnostic::error(ErrorCode::E1001)
        .with_message("first")
        .with_label(Span::new(0, 1), "here");
    assert!(queue.add_with_severity(again, 1, 1, DiagnosticSeverity::Hard));
    assert_eq!(queue.flush().len(), 1);
}

// `too_many_errors` factory shape.

#[test]
fn test_too_many_errors_diagnostic_shape() {
    let diag = too_many_errors(10, Span::new(0, 1));

    assert_eq!(diag.code, ErrorCode::E9002);
    assert!(diag.is_error());
    assert!(diag.message.contains("10"));
    assert_eq!(diag.labels.len(), 1);
    assert_eq!(diag.notes.len(), 1);
    assert!(diag.notes[0].contains("--error-limit"));
}
