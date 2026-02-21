use crate::errors::{EvalError, EvalErrorKind};
use ori_diagnostic::ErrorCode;
use ori_ir::{BinaryOp, Span};

use crate::errors::{BacktraceFrame, EvalBacktrace, EvalNote};

#[test]
fn division_by_zero_maps_to_e6001() {
    let err = crate::division_by_zero();
    let diag = err.to_diagnostic();
    assert_eq!(diag.code, ErrorCode::E6001);
    assert!(diag.message.contains("division by zero"));
}

#[test]
fn undefined_variable_maps_to_e6020() {
    let err = crate::undefined_variable("x");
    let diag = err.to_diagnostic();
    assert_eq!(diag.code, ErrorCode::E6020);
    assert!(diag.message.contains('x'));
}

#[test]
fn span_produces_primary_label() {
    let err = crate::division_by_zero().with_span(Span::new(10, 20));
    let diag = err.to_diagnostic();
    assert_eq!(diag.labels.len(), 1);
    assert_eq!(diag.labels[0].span, Span::new(10, 20));
}

#[test]
fn no_span_produces_no_label() {
    let err = crate::division_by_zero();
    let diag = err.to_diagnostic();
    assert!(diag.labels.is_empty());
}

#[test]
fn notes_are_carried_over() {
    let err = crate::division_by_zero().with_note(EvalNote {
        message: "check your denominators".to_string(),
        span: None,
    });
    let diag = err.to_diagnostic();
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
    let err = crate::division_by_zero().with_backtrace(bt);
    let diag = err.to_diagnostic();
    assert!(diag.notes.iter().any(|n| n.contains("call stack")));
}

#[test]
fn immutable_binding_has_suggestion() {
    let err = crate::cannot_assign_immutable("x");
    let diag = err.to_diagnostic();
    assert!(!diag.suggestions.is_empty());
    assert!(diag.suggestions[0].contains("mut x"));
}

#[test]
fn custom_error_maps_to_e6099() {
    let err = EvalError::new("something went wrong");
    let diag = err.to_diagnostic();
    assert_eq!(diag.code, ErrorCode::E6099);
}

#[test]
fn stack_overflow_maps_to_e6031() {
    let err = crate::recursion_limit_exceeded(200);
    let diag = err.to_diagnostic();
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
        let code = kind.error_code();
        assert!(
            codes.insert(code),
            "duplicate error code {code} for kind {kind:?}"
        );
    }
}
