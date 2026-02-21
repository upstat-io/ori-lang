use super::*;
use crate::eval::EvalErrorSnapshot;
use crate::ir::Span;
use ori_diagnostic::ErrorCode;

// Enriched snapshot-to-diagnostic tests

#[test]
fn snapshot_enriches_span_with_file_line_col() {
    // Source: "let x = 1 / 0" — the "/ 0" starts at offset 10
    let source = "let x = 1 / 0";
    let snapshot = EvalErrorSnapshot {
        message: "division by zero".to_string(),
        kind_name: "DivisionByZero".to_string(),
        error_code: ErrorCode::E6001,
        span: Some(Span::new(10, 13)),
        backtrace: vec![],
        notes: vec![],
    };

    let diag = snapshot_to_diagnostic(&snapshot, source, "main.ori");
    assert_eq!(diag.code, ErrorCode::E6001);
    assert_eq!(diag.labels.len(), 1);
    assert!(diag.labels[0].message.contains("main.ori:1:11"));
}

#[test]
fn snapshot_enriches_multiline_span() {
    let source = "let x = 1\nlet y = 2 / 0";
    // "/ 0" at line 2, col 11 (offset 20)
    let snapshot = EvalErrorSnapshot {
        message: "division by zero".to_string(),
        kind_name: "DivisionByZero".to_string(),
        error_code: ErrorCode::E6001,
        span: Some(Span::new(20, 23)),
        backtrace: vec![],
        notes: vec![],
    };

    let diag = snapshot_to_diagnostic(&snapshot, source, "math.ori");
    assert!(diag.labels[0].message.contains("math.ori:2:11"));
}

#[test]
fn snapshot_enriches_backtrace_with_file_line() {
    // Source layout:
    //   offset 0:  "fn foo() =\n"  (line 1)
    //   offset 12: "  bar()\n"     (line 2, bar() call at offset 13)
    //   offset 20: "fn bar() =\n"  (line 3)
    //   offset 32: "  1 / 0"       (line 4, "/" at offset 34)
    let source = "fn foo() =\n  bar()\nfn bar() =\n  1 / 0";
    let snapshot = EvalErrorSnapshot {
        message: "division by zero".to_string(),
        kind_name: "DivisionByZero".to_string(),
        error_code: ErrorCode::E6001,
        span: Some(Span::new(34, 37)),
        backtrace: vec![
            ("bar".to_string(), Some(Span::new(34, 37))),
            ("foo".to_string(), Some(Span::new(13, 18))),
        ],
        notes: vec![],
    };

    let diag = snapshot_to_diagnostic(&snapshot, source, "test.ori");
    let bt_note = diag.notes.iter().find(|n| n.contains("call stack"));
    assert!(bt_note.is_some());
    let bt = bt_note.unwrap();
    assert!(bt.contains("0: bar at test.ori:4:5"), "actual: {bt}");
    assert!(bt.contains("1: foo at test.ori:2:3"), "actual: {bt}");
}

#[test]
fn snapshot_no_span_produces_no_label() {
    let snapshot = EvalErrorSnapshot {
        message: "runtime error".to_string(),
        kind_name: "Custom".to_string(),
        error_code: ErrorCode::E6099,
        span: None,
        backtrace: vec![],
        notes: vec![],
    };

    let diag = snapshot_to_diagnostic(&snapshot, "", "test.ori");
    assert!(diag.labels.is_empty());
}

#[test]
fn snapshot_preserves_notes() {
    let snapshot = EvalErrorSnapshot {
        message: "error".to_string(),
        kind_name: "Custom".to_string(),
        error_code: ErrorCode::E6099,
        span: None,
        backtrace: vec![],
        notes: vec!["hint: check input".to_string()],
    };

    let diag = snapshot_to_diagnostic(&snapshot, "", "test.ori");
    assert!(diag.notes.iter().any(|n| n.contains("check input")));
}
