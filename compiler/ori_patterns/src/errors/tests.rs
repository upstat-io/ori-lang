use super::*;

// Kind → message round-trip

#[test]
fn division_by_zero_has_correct_kind() {
    let err = division_by_zero();
    assert_eq!(err.kind, EvalErrorKind::DivisionByZero);
    assert_eq!(err.message, "division by zero");
}

#[test]
fn modulo_by_zero_has_correct_kind() {
    let err = modulo_by_zero();
    assert_eq!(err.kind, EvalErrorKind::ModuloByZero);
    assert_eq!(err.message, "modulo by zero");
}

#[test]
fn integer_overflow_has_correct_kind() {
    let err = integer_overflow("addition");
    assert_eq!(
        err.kind,
        EvalErrorKind::IntegerOverflow {
            operation: "addition".to_string()
        }
    );
    assert_eq!(err.message, "integer overflow in addition");
}

#[test]
fn undefined_variable_has_correct_kind() {
    let err = undefined_variable("x");
    assert_eq!(
        err.kind,
        EvalErrorKind::UndefinedVariable {
            name: "x".to_string()
        }
    );
    assert_eq!(err.message, "undefined variable: x");
}

#[test]
fn undefined_function_has_correct_kind() {
    let err = undefined_function("foo");
    assert_eq!(
        err.kind,
        EvalErrorKind::UndefinedFunction {
            name: "foo".to_string()
        }
    );
    assert_eq!(err.message, "undefined function: @foo");
}

#[test]
fn arity_mismatch_with_name() {
    let err = wrong_arg_count("push", 1, 2);
    assert_eq!(
        err.kind,
        EvalErrorKind::ArityMismatch {
            name: "push".to_string(),
            expected: 1,
            got: 2
        }
    );
    assert_eq!(err.message, "push expects 1 argument, got 2");
}

#[test]
fn arity_mismatch_without_name() {
    let err = wrong_function_args(3, 1);
    assert_eq!(
        err.kind,
        EvalErrorKind::ArityMismatch {
            name: String::new(),
            expected: 3,
            got: 1
        }
    );
    assert_eq!(err.message, "expected 3 arguments, got 1");
}

#[test]
fn stack_overflow_has_correct_kind() {
    let err = recursion_limit_exceeded(200);
    assert_eq!(err.kind, EvalErrorKind::StackOverflow { depth: 200 });
    assert_eq!(err.message, "maximum recursion depth exceeded (limit: 200)");
}

#[test]
fn non_exhaustive_match_has_correct_kind() {
    let err = non_exhaustive_match();
    assert_eq!(err.kind, EvalErrorKind::NonExhaustiveMatch);
    assert_eq!(err.message, "non-exhaustive match");
}

#[test]
fn not_implemented_has_correct_kind() {
    let err = index_assignment_not_implemented();
    assert!(matches!(err.kind, EvalErrorKind::NotImplemented { .. }));
    assert!(err.message.contains("not yet implemented"));
    assert!(err.message.contains("list.set"));
}

#[test]
fn custom_kind_for_new() {
    let err = EvalError::new("something broke");
    assert_eq!(
        err.kind,
        EvalErrorKind::Custom {
            message: "something broke".to_string()
        }
    );
    assert_eq!(err.message, "something broke");
}

// Builder methods

#[test]
fn with_span_sets_span() {
    let span = Span::new(10, 20);
    let err = division_by_zero().with_span(span);
    assert_eq!(err.span, Some(span));
}

#[test]
fn with_backtrace_sets_backtrace() {
    let bt = EvalBacktrace::new(vec![BacktraceFrame {
        name: "foo".to_string(),
        span: None,
    }]);
    let err = division_by_zero().with_backtrace(bt);
    assert!(err.backtrace.is_some());
    assert_eq!(err.backtrace.as_ref().map(EvalBacktrace::len), Some(1));
}

#[test]
fn with_note_adds_note() {
    let err = division_by_zero().with_note(EvalNote::new("denominator was 0"));
    assert_eq!(err.notes.len(), 1);
    assert_eq!(err.notes[0].message, "denominator was 0");
}

// Backtrace display

#[test]
fn empty_backtrace_display() {
    let bt = EvalBacktrace::default();
    assert!(bt.is_empty());
    assert_eq!(bt.display(), "");
}

#[test]
fn backtrace_display_with_frames() {
    let bt = EvalBacktrace::new(vec![
        BacktraceFrame {
            name: "bar".to_string(),
            span: Some(Span::new(100, 110)),
        },
        BacktraceFrame {
            name: "foo".to_string(),
            span: None,
        },
    ]);
    let display = bt.display();
    assert!(display.contains("0: bar"));
    assert!(display.contains("1: foo"));
}

// Kind display round-trip: verify Display matches message for all factory funcs

#[test]
fn kind_display_matches_message() {
    let errors: Vec<EvalError> = vec![
        division_by_zero(),
        modulo_by_zero(),
        integer_overflow("mul"),
        no_such_method("len", "int"),
        wrong_arg_count("push", 1, 3),
        wrong_function_args(2, 0),
        undefined_variable("x"),
        undefined_function("main"),
        undefined_const("PI"),
        not_callable("int"),
        index_out_of_bounds(5),
        key_not_found("name"),
        no_field_on_struct("age"),
        non_exhaustive_match(),
        cannot_assign_immutable("x"),
        recursion_limit_exceeded(100),
    ];
    for err in &errors {
        assert_eq!(
            err.message,
            err.kind.to_string(),
            "message/kind mismatch for {:?}",
            err.kind
        );
    }
}

// variant_name()

#[test]
fn variant_name_returns_stable_strings() {
    assert_eq!(
        EvalErrorKind::DivisionByZero.variant_name(),
        "DivisionByZero"
    );
    assert_eq!(EvalErrorKind::ModuloByZero.variant_name(), "ModuloByZero");
    assert_eq!(
        EvalErrorKind::IntegerOverflow {
            operation: "add".into()
        }
        .variant_name(),
        "IntegerOverflow"
    );
    assert_eq!(
        EvalErrorKind::TypeMismatch {
            expected: "int".into(),
            got: "str".into()
        }
        .variant_name(),
        "TypeMismatch"
    );
    assert_eq!(
        EvalErrorKind::UndefinedVariable { name: "x".into() }.variant_name(),
        "UndefinedVariable"
    );
    assert_eq!(
        EvalErrorKind::NonExhaustiveMatch.variant_name(),
        "NonExhaustiveMatch"
    );
    assert_eq!(
        EvalErrorKind::ConstEvalBudgetExceeded.variant_name(),
        "ConstEvalBudgetExceeded"
    );
    assert_eq!(
        EvalErrorKind::Custom {
            message: "test".into()
        }
        .variant_name(),
        "Custom"
    );
}

// ControlAction tests

#[test]
fn control_action_break_carries_value() {
    let action = ControlAction::Break(Value::int(42));
    assert!(!action.is_error());
    if let ControlAction::Break(v) = action {
        assert_eq!(v, Value::int(42));
    } else {
        panic!("expected Break");
    }
}

#[test]
fn control_action_continue_carries_value() {
    let action = ControlAction::Continue(Value::Void);
    assert!(!action.is_error());
    assert!(matches!(action, ControlAction::Continue(Value::Void)));
}

#[test]
fn control_action_propagate_carries_value() {
    let action = ControlAction::Propagate(Value::None);
    assert!(!action.is_error());
    assert!(matches!(action, ControlAction::Propagate(Value::None)));
}

#[test]
fn control_action_error_is_error() {
    let action: ControlAction = EvalError::new("test").into();
    assert!(action.is_error());
}

#[test]
fn control_action_from_eval_error() {
    let err = division_by_zero();
    let action: ControlAction = err.into();
    assert!(action.is_error());
    if let ControlAction::Error(e) = action {
        assert_eq!(e.kind, EvalErrorKind::DivisionByZero);
    } else {
        panic!("expected Error");
    }
}

#[test]
fn control_action_into_eval_error_roundtrip() {
    let err = division_by_zero();
    let msg = err.message.clone();
    let action: ControlAction = err.into();
    let recovered = action.into_eval_error();
    assert_eq!(recovered.message, msg);
}

#[test]
fn control_action_into_eval_error_from_break() {
    let action = ControlAction::Break(Value::int(5));
    let err = action.into_eval_error();
    assert!(err.message.contains("break"));
}

#[test]
fn control_action_with_span_if_error_attaches_span() {
    let span = Span::new(10, 20);
    let action: ControlAction = EvalError::new("test").into();
    let action = action.with_span_if_error(span);
    if let ControlAction::Error(e) = action {
        assert_eq!(e.span, Some(span));
    } else {
        panic!("expected Error");
    }
}

#[test]
fn control_action_with_span_if_error_ignores_control_flow() {
    let span = Span::new(10, 20);
    let action = ControlAction::Break(Value::Void);
    let action = action.with_span_if_error(span);
    // Break should pass through unchanged
    assert!(matches!(action, ControlAction::Break(Value::Void)));
}
