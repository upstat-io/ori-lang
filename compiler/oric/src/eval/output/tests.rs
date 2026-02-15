use super::*;
use crate::ir::SharedInterner;

#[test]
fn test_eval_output_from_value() {
    let interner = SharedInterner::default();

    assert_eq!(
        EvalOutput::from_value(&Value::int(42), &interner),
        EvalOutput::Int(42)
    );
    assert_eq!(
        EvalOutput::from_value(&Value::Bool(true), &interner),
        EvalOutput::Bool(true)
    );
    assert_eq!(
        EvalOutput::from_value(&Value::Void, &interner),
        EvalOutput::Void
    );
    assert_eq!(
        EvalOutput::from_value(&Value::None, &interner),
        EvalOutput::None
    );
}

#[test]
fn test_eval_output_display() {
    let interner = SharedInterner::default();
    assert_eq!(EvalOutput::Int(42).display(&interner), "42");
    assert_eq!(EvalOutput::Bool(true).display(&interner), "true");
    assert_eq!(EvalOutput::Void.display(&interner), "void");
    assert_eq!(EvalOutput::None.display(&interner), "None");
    assert_eq!(
        EvalOutput::Some(Box::new(EvalOutput::Int(1))).display(&interner),
        "Some(1)"
    );
    assert_eq!(
        EvalOutput::Ok(Box::new(EvalOutput::Int(1))).display(&interner),
        "Ok(1)"
    );
    assert_eq!(
        EvalOutput::List(vec![EvalOutput::Int(1), EvalOutput::Int(2)]).display(&interner),
        "[1, 2]"
    );
    assert_eq!(
        EvalOutput::Tuple(vec![EvalOutput::Int(1), EvalOutput::Bool(true)]).display(&interner),
        "(1, true)"
    );
}

#[test]
fn test_eval_output_variant_display() {
    let interner = SharedInterner::default();
    let type_name = interner.intern("Option");
    let some_name = interner.intern("Some");
    let none_name = interner.intern("None");

    // Variant without fields
    let none_variant = EvalOutput::Variant {
        type_name,
        variant_name: none_name,
        fields: vec![],
    };
    assert_eq!(none_variant.display(&interner), "Option::None");

    // Variant with fields
    let some_variant = EvalOutput::Variant {
        type_name,
        variant_name: some_name,
        fields: vec![EvalOutput::Int(42)],
    };
    assert_eq!(some_variant.display(&interner), "Option::Some(42)");
}

#[test]
fn test_eval_output_equality() {
    assert_eq!(EvalOutput::Int(42), EvalOutput::Int(42));
    assert_ne!(EvalOutput::Int(42), EvalOutput::Int(43));
    assert_ne!(EvalOutput::Int(42), EvalOutput::Bool(true));

    assert_eq!(
        EvalOutput::List(vec![EvalOutput::Int(1)]),
        EvalOutput::List(vec![EvalOutput::Int(1)])
    );
}

#[test]
fn test_module_eval_result() {
    let success = ModuleEvalResult::success(EvalOutput::Int(42));
    assert!(success.is_success());
    assert!(!success.is_failure());
    assert!(success.eval_error.is_none());

    let failure = ModuleEvalResult::failure("test error".to_string());
    assert!(!failure.is_success());
    assert!(failure.is_failure());
    assert!(failure.eval_error.is_none());
}

#[test]
fn test_runtime_error_preserves_snapshot() {
    let err = ori_patterns::division_by_zero().with_span(Span::new(10, 20));
    let result = ModuleEvalResult::runtime_error(&err);

    assert!(result.is_failure());
    assert!(result.error.as_ref().unwrap().contains("division by zero"));

    let snapshot = result.eval_error.as_ref().unwrap();
    assert_eq!(snapshot.span, Some(Span::new(10, 20)));
    assert_eq!(snapshot.kind_name, "DivisionByZero");
    assert_eq!(snapshot.error_code, ErrorCode::E6001);
    assert!(snapshot.message.contains("division by zero"));
}

#[test]
fn test_snapshot_captures_backtrace() {
    use ori_patterns::{BacktraceFrame, EvalBacktrace};

    let bt = EvalBacktrace::new(vec![
        BacktraceFrame {
            name: "foo".to_string(),
            span: Some(Span::new(5, 10)),
        },
        BacktraceFrame {
            name: "bar".to_string(),
            span: None,
        },
    ]);
    let err = ori_patterns::division_by_zero().with_backtrace(bt);
    let snapshot = EvalErrorSnapshot::from_eval_error(&err);

    assert_eq!(snapshot.backtrace.len(), 2);
    assert_eq!(snapshot.backtrace[0].0, "foo");
    assert_eq!(snapshot.backtrace[0].1, Some(Span::new(5, 10)));
    assert_eq!(snapshot.backtrace[1].0, "bar");
    assert_eq!(snapshot.backtrace[1].1, None);
}

#[test]
fn test_snapshot_captures_notes() {
    use ori_patterns::EvalNote;

    let err = ori_patterns::division_by_zero()
        .with_note(EvalNote {
            message: "check denominator".to_string(),
            span: None,
        })
        .with_note(EvalNote {
            message: "second note".to_string(),
            span: Some(Span::new(0, 5)),
        });
    let snapshot = EvalErrorSnapshot::from_eval_error(&err);

    assert_eq!(snapshot.notes.len(), 2);
    assert_eq!(snapshot.notes[0], "check denominator");
    assert_eq!(snapshot.notes[1], "second note");
}

#[test]
fn test_snapshot_salsa_traits() {
    // Verify Clone + Eq + Hash work (required for Salsa)
    use std::collections::HashSet;

    let err = ori_patterns::division_by_zero().with_span(Span::new(0, 5));
    let snapshot = EvalErrorSnapshot::from_eval_error(&err);
    let cloned = snapshot.clone();
    assert_eq!(snapshot, cloned);

    let mut set = HashSet::new();
    set.insert(snapshot.clone());
    assert!(set.contains(&cloned));
}
