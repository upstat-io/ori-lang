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
fn recurse_returns_base_when_condition_true() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let cond_name = interner.intern("condition");
    let base_name = interner.intern("base");
    let step_name = interner.intern("step");
    let props = vec![
        NamedExpr {
            name: cond_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        },
        NamedExpr {
            name: base_name,
            value: ExprId::new(1),
            span: Span::new(0, 0),
        },
        NamedExpr {
            name: step_name,
            value: ExprId::new(2),
            span: Span::new(0, 0),
        },
    ];

    let mut exec = MockPatternExecutor::new()
        .with_expr(ExprId::new(0), Value::Bool(true)) // condition is true
        .with_expr(ExprId::new(1), Value::int(42)) // base value
        .with_expr(ExprId::new(2), Value::int(100)); // step value

    let ctx = make_ctx(&interner, &arena, &props);
    let result = RecursePattern.evaluate(&ctx, &mut exec).unwrap();

    assert_eq!(result, Value::int(42));
}

#[test]
fn recurse_returns_step_when_condition_false() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let cond_name = interner.intern("condition");
    let base_name = interner.intern("base");
    let step_name = interner.intern("step");
    let props = vec![
        NamedExpr {
            name: cond_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        },
        NamedExpr {
            name: base_name,
            value: ExprId::new(1),
            span: Span::new(0, 0),
        },
        NamedExpr {
            name: step_name,
            value: ExprId::new(2),
            span: Span::new(0, 0),
        },
    ];

    let mut exec = MockPatternExecutor::new()
        .with_expr(ExprId::new(0), Value::Bool(false)) // condition is false
        .with_expr(ExprId::new(1), Value::int(42)) // base value
        .with_expr(ExprId::new(2), Value::int(100)); // step value

    let ctx = make_ctx(&interner, &arena, &props);
    let result = RecursePattern.evaluate(&ctx, &mut exec).unwrap();

    assert_eq!(result, Value::int(100));
}

#[test]
fn recurse_pattern_name() {
    assert_eq!(RecursePattern.name(), "recurse");
}

#[test]
fn recurse_required_props() {
    assert_eq!(
        RecursePattern.required_props(),
        &["condition", "base", "step"]
    );
}

#[test]
fn recurse_optional_props() {
    assert_eq!(RecursePattern.optional_props(), &["memo"]);
}

#[test]
fn recurse_has_scoped_bindings_for_self() {
    let bindings = RecursePattern.scoped_bindings();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].name, "self");
    assert_eq!(bindings[0].for_props, &["step"]);
}
