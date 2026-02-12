//! Runtime/eval error to diagnostic conversion.
//!
//! Converts `EvalError` (from `ori_patterns`) into `Diagnostic` (from `ori_diagnostic`)
//! using E6xxx error codes. This conversion lives in `oric` due to the orphan rule:
//! `EvalError` is in `ori_patterns`, `Diagnostic` is in `ori_diagnostic`, and neither
//! crate owns the other.
//!
//! # Error Code Ranges (E6xxx)
//!
//! - E6001–E6009: Arithmetic (division by zero, overflow)
//! - E6010–E6019: Type/operator errors
//! - E6020–E6029: Access errors (undefined variable/function/field/method)
//! - E6030–E6039: Function call errors (arity, stack overflow, not callable)
//! - E6040–E6049: Pattern/match errors
//! - E6050–E6059: Assertion/test errors
//! - E6060–E6069: Capability errors
//! - E6070–E6079: Const-eval errors
//! - E6080–E6089: Not-implemented errors
//! - E6099: Custom/uncategorized

use std::fmt::Write;

use crate::eval::EvalErrorSnapshot;
use ori_diagnostic::span_utils::LineOffsetTable;
use ori_diagnostic::{Diagnostic, ErrorCode};
use ori_patterns::{EvalError, EvalErrorKind};

/// Convert an `EvalError` into a `Diagnostic`.
///
/// Maps `EvalErrorKind` to an E6xxx error code and constructs a diagnostic
/// with the error message, primary span label, notes, and backtrace context.
pub fn eval_error_to_diagnostic(err: &EvalError) -> Diagnostic {
    let code = error_code_for_kind(&err.kind);
    let mut diag = Diagnostic::error(code).with_message(&err.message);

    // Add primary label at the error span
    if let Some(span) = err.span {
        diag = diag.with_label(span, label_for_kind(&err.kind));
    }

    // Add context notes from the error
    for note in &err.notes {
        diag = diag.with_note(&note.message);
    }

    // Add backtrace as a note (if present)
    if let Some(ref bt) = err.backtrace {
        if !bt.is_empty() {
            diag = diag.with_note(format!("call stack:\n{bt}"));
        }
    }

    // Add suggestions for common fixable errors
    if let Some(suggestion) = suggestion_for_kind(&err.kind) {
        diag = diag.with_suggestion(suggestion);
    }

    diag
}

/// Convert an `EvalErrorSnapshot` into a `Diagnostic` with enriched file/line info.
///
/// Unlike [`eval_error_to_diagnostic`] which works with raw `EvalError` (and its
/// `EvalErrorKind` for error code mapping), this function works with the Salsa-compatible
/// snapshot and enriches backtrace spans with `file:line:col` using `LineOffsetTable`.
///
/// Falls back to byte offsets if source is unavailable.
pub fn snapshot_to_diagnostic(
    snapshot: &EvalErrorSnapshot,
    source: &str,
    file_path: &str,
) -> Diagnostic {
    let mut diag = Diagnostic::error(snapshot.error_code).with_message(&snapshot.message);

    let table = LineOffsetTable::build(source);

    // Add primary label at the error span
    if let Some(span) = snapshot.span {
        let (line, col) = table.offset_to_line_col(source, span.start);
        diag = diag.with_label(span, format!("runtime error at {file_path}:{line}:{col}"));
    }

    // Add context notes
    for note in &snapshot.notes {
        diag = diag.with_note(note);
    }

    // Add enriched backtrace as a note
    if !snapshot.backtrace.is_empty() {
        let mut bt_lines = String::from("call stack:");
        for (i, (name, span)) in snapshot.backtrace.iter().enumerate() {
            let _ = write!(bt_lines, "\n  {i}: {name}");
            if let Some(span) = span {
                let (line, col) = table.offset_to_line_col(source, span.start);
                let _ = write!(bt_lines, " at {file_path}:{line}:{col}");
            }
        }
        diag = diag.with_note(bt_lines);
    }

    diag
}

/// Map an `EvalErrorKind` to its corresponding `ErrorCode`.
pub(crate) fn error_code_for_kind(kind: &EvalErrorKind) -> ErrorCode {
    match kind {
        // Arithmetic
        EvalErrorKind::DivisionByZero => ErrorCode::E6001,
        EvalErrorKind::ModuloByZero => ErrorCode::E6002,
        EvalErrorKind::IntegerOverflow { .. } => ErrorCode::E6003,

        // Type/Operator
        EvalErrorKind::TypeMismatch { .. } => ErrorCode::E6010,
        EvalErrorKind::InvalidBinaryOp { .. } => ErrorCode::E6011,
        EvalErrorKind::BinaryTypeMismatch { .. } => ErrorCode::E6012,

        // Access
        EvalErrorKind::UndefinedVariable { .. } => ErrorCode::E6020,
        EvalErrorKind::UndefinedFunction { .. } => ErrorCode::E6021,
        EvalErrorKind::UndefinedConst { .. } => ErrorCode::E6022,
        EvalErrorKind::UndefinedField { .. } => ErrorCode::E6023,
        EvalErrorKind::UndefinedMethod { .. } => ErrorCode::E6024,
        EvalErrorKind::IndexOutOfBounds { .. } => ErrorCode::E6025,
        EvalErrorKind::KeyNotFound { .. } => ErrorCode::E6026,
        EvalErrorKind::ImmutableBinding { .. } => ErrorCode::E6027,

        // Function
        EvalErrorKind::ArityMismatch { .. } => ErrorCode::E6030,
        EvalErrorKind::StackOverflow { .. } => ErrorCode::E6031,
        EvalErrorKind::NotCallable { .. } => ErrorCode::E6032,

        // Pattern
        EvalErrorKind::NonExhaustiveMatch => ErrorCode::E6040,

        // Assertion/Test
        EvalErrorKind::AssertionFailed { .. } => ErrorCode::E6050,
        EvalErrorKind::PanicCalled { .. } => ErrorCode::E6051,

        // Capability
        EvalErrorKind::MissingCapability { .. } => ErrorCode::E6060,

        // Const Eval
        EvalErrorKind::ConstEvalBudgetExceeded => ErrorCode::E6070,

        // Not Implemented
        EvalErrorKind::NotImplemented { .. } => ErrorCode::E6080,

        // Custom/catch-all
        EvalErrorKind::Custom { .. } => ErrorCode::E6099,
    }
}

/// Produce a concise label for the primary span.
fn label_for_kind(kind: &EvalErrorKind) -> &'static str {
    match kind {
        EvalErrorKind::DivisionByZero => "division by zero here",
        EvalErrorKind::ModuloByZero => "modulo by zero here",
        EvalErrorKind::IntegerOverflow { .. } => "overflow occurred here",
        EvalErrorKind::TypeMismatch { .. } => "type mismatch",
        EvalErrorKind::InvalidBinaryOp { .. } => "operator not supported",
        EvalErrorKind::BinaryTypeMismatch { .. } => "mismatched types",
        EvalErrorKind::UndefinedVariable { .. } => "not found in this scope",
        EvalErrorKind::UndefinedFunction { .. } => "function not found",
        EvalErrorKind::UndefinedConst { .. } => "constant not found",
        EvalErrorKind::UndefinedField { .. } => "field not found",
        EvalErrorKind::UndefinedMethod { .. } => "method not found",
        EvalErrorKind::IndexOutOfBounds { .. } => "index out of bounds",
        EvalErrorKind::KeyNotFound { .. } => "key not found",
        EvalErrorKind::ImmutableBinding { .. } => "cannot assign to immutable binding",
        EvalErrorKind::ArityMismatch { .. } => "wrong number of arguments",
        EvalErrorKind::StackOverflow { .. } => "recursion limit exceeded",
        EvalErrorKind::NotCallable { .. } => "not callable",
        EvalErrorKind::NonExhaustiveMatch => "non-exhaustive match",
        EvalErrorKind::AssertionFailed { .. } => "assertion failed",
        EvalErrorKind::PanicCalled { .. } => "panic",
        EvalErrorKind::MissingCapability { .. } => "missing capability",
        EvalErrorKind::ConstEvalBudgetExceeded => "budget exceeded",
        EvalErrorKind::NotImplemented { .. } => "not implemented",
        EvalErrorKind::Custom { .. } => "runtime error",
    }
}

/// Produce an actionable suggestion for fixable errors.
fn suggestion_for_kind(kind: &EvalErrorKind) -> Option<String> {
    match kind {
        EvalErrorKind::DivisionByZero => Some("add a zero check before dividing".to_string()),
        EvalErrorKind::ImmutableBinding { name } => Some(format!(
            "declare `{name}` as mutable with `mut {name} = ...`"
        )),
        EvalErrorKind::NonExhaustiveMatch => {
            Some("add a wildcard `_` arm to cover remaining cases".to_string())
        }
        EvalErrorKind::NotImplemented { suggestion, .. } if !suggestion.is_empty() => {
            Some(suggestion.clone())
        }
        EvalErrorKind::MissingCapability { capability } => {
            Some(format!("add `uses {capability}` to the function signature"))
        }
        _ => None,
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use crate::eval::EvalErrorSnapshot;
    use ori_ir::Span;
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
            EvalErrorKind::TypeMismatch {
                expected: String::new(),
                got: String::new(),
            },
            EvalErrorKind::InvalidBinaryOp {
                type_name: String::new(),
                op: ori_ir::BinaryOp::Add,
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
}
