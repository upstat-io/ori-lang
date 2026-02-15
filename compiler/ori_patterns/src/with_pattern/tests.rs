use super::*;
use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

fn make_ctx<'a>(
    interner: &'a SharedInterner,
    arena: &'a ExprArena,
    props: &'a [NamedExpr],
) -> EvalContext<'a> {
    EvalContext::new(interner, arena, props)
}

#[test]
fn with_pattern_name() {
    assert_eq!(WithPattern.name(), "with");
}

#[test]
fn with_required_props() {
    assert_eq!(WithPattern.required_props(), &["acquire", "action"]);
}

#[test]
fn with_optional_props() {
    assert_eq!(WithPattern.optional_props(), &["release"]);
}

#[test]
fn with_returns_action_result() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let acquire_name = interner.intern("acquire");
    let action_name = interner.intern("action");
    let props = vec![
        NamedExpr {
            name: acquire_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        },
        NamedExpr {
            name: action_name,
            value: ExprId::new(1),
            span: Span::new(0, 0),
        },
    ];

    // Mock returns resource for acquire, action function, and call result
    let mut exec = MockPatternExecutor::new()
        .with_expr(ExprId::new(0), Value::string("resource"))
        .with_expr(ExprId::new(1), Value::Void) // Action function placeholder
        .with_call_results(vec![Value::int(42)]); // Action call returns 42

    let ctx = make_ctx(&interner, &arena, &props);
    let result = WithPattern.evaluate(&ctx, &mut exec).unwrap();

    assert_eq!(result, Value::int(42));
}
