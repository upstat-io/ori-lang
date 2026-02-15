use super::*;
use crate::eval::EvalErrorSnapshot;
use crate::ir::{BinaryOp, Span};
use ori_patterns::{BacktraceFrame, EvalBacktrace, EvalNote};

#[test]
fn division_by_zero_maps_to_e6001() {
    let err = ori_patterns::division_by_zero();
    let diag = eval_error_to_diagnostic(&err);
    assert_eq!(diag.code, ErrorCode::E6001);
    assert!(diag.message.contains("division by zero"));
}

#[test]
fn undefined_variable_maps_to_e6020() {
    let err = ori_patterns::undefined_variable("x");
    let diag = eval_error_to_diagnostic(&err);
    assert_eq!(diag.code, ErrorCode::E6020);
    assert!(diag.message.contains('x'));
}

#[test]
fn span_produces_primary_label() {
    let err = ori_patterns::division_by_zero().with_span(Span::new(10, 20));
    let diag = eval_error_to_diagnostic(&err);
    assert_eq!(diag.labels.len(), 1);
    assert_eq!(diag.labels[0].span, Span::new(10, 20));
}

#[test]
fn no_span_produces_no_label() {
    let err = ori_patterns::division_by_zero();
    let diag = eval_error_to_diagnostic(&err);
    assert!(diag.labels.is_empty());
}

#[test]
fn notes_are_carried_over() {
    let err = ori_patterns::division_by_zero().with_note(EvalNote {
        message: "check your denominators".to_string(),
        span: None,
    });
    let diag = eval_error_to_diagnostic(&err);
    assert!(diag.notes.iter().any(|n| n.contains("denominators")));
}

#[test]
fn backtrace_produces_note() {
    let bt = EvalBacktrace::new(vec![
        BacktraceFrame {
            name: "foo".to_string(),
            span: None,
        },
        BacktraceFrame {
            name: "bar".to_string(),
            span: Some(Span::new(5, 10)),
        },
    ]);
    let err = ori_patterns::division_by_zero().with_backtrace(bt);
    let diag = eval_error_to_diagnostic(&err);
    assert!(diag.notes.iter().any(|n| n.contains("call stack")));
}

#[test]
fn immutable_binding_has_suggestion() {
    let err = ori_eval::errors::cannot_assign_immutable("x");
    let diag = eval_error_to_diagnostic(&err);
    assert!(!diag.suggestions.is_empty());
    assert!(diag.suggestions[0].contains("mut x"));
}

#[test]
fn custom_error_maps_to_e6099() {
    let err = EvalError::new("something went wrong");
    let diag = eval_error_to_diagnostic(&err);
    assert_eq!(diag.code, ErrorCode::E6099);
}

#[test]
fn stack_overflow_maps_to_e6031() {
    let err = ori_patterns::recursion_limit_exceeded(200);
    let diag = eval_error_to_diagnostic(&err);
    assert_eq!(diag.code, ErrorCode::E6031);
    assert!(diag.message.contains("200"));
}

#[test]
fn all_kinds_have_unique_codes() {
    use std::collections::HashSet;
    let kinds = vec![
        EvalErrorKind::DivisionByZero,
        EvalErrorKind::ModuloByZero,
        EvalErrorKind::IntegerOverflow {
            operation: String::new(),
        },
        EvalErrorKind::SizeWouldBeNegative,
        EvalErrorKind::SizeNegativeMultiply,
        EvalErrorKind::SizeNegativeDivide,
        EvalErrorKind::TypeMismatch {
            expected: String::new(),
            got: String::new(),
        },
        EvalErrorKind::InvalidBinaryOp {
            type_name: String::new(),
            op: BinaryOp::Add,
        },
        EvalErrorKind::BinaryTypeMismatch {
            left: String::new(),
            right: String::new(),
        },
        EvalErrorKind::UndefinedVariable {
            name: String::new(),
        },
        EvalErrorKind::UndefinedFunction {
            name: String::new(),
        },
        EvalErrorKind::UndefinedConst {
            name: String::new(),
        },
        EvalErrorKind::UndefinedField {
            field: String::new(),
        },
        EvalErrorKind::UndefinedMethod {
            method: String::new(),
            type_name: String::new(),
        },
        EvalErrorKind::IndexOutOfBounds { index: 0 },
        EvalErrorKind::KeyNotFound { key: String::new() },
        EvalErrorKind::ImmutableBinding {
            name: String::new(),
        },
        EvalErrorKind::ArityMismatch {
            name: String::new(),
            expected: 0,
            got: 0,
        },
        EvalErrorKind::StackOverflow { depth: 0 },
        EvalErrorKind::NotCallable {
            type_name: String::new(),
        },
        EvalErrorKind::NonExhaustiveMatch,
        EvalErrorKind::AssertionFailed {
            message: String::new(),
        },
        EvalErrorKind::PanicCalled {
            message: String::new(),
        },
        EvalErrorKind::MissingCapability {
            capability: String::new(),
        },
        EvalErrorKind::ConstEvalBudgetExceeded,
        EvalErrorKind::NotImplemented {
            feature: String::new(),
            suggestion: String::new(),
        },
        EvalErrorKind::Custom {
            message: String::new(),
        },
    ];

    let mut codes = HashSet::new();
    for kind in &kinds {
        let code = error_code_for_kind(kind);
        assert!(
            codes.insert(code),
            "duplicate error code {code} for kind {kind:?}"
        );
    }
}

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
