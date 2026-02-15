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
fn spawn_empty_list_returns_void() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let tasks_name = interner.intern("tasks");
    let props = vec![NamedExpr {
        name: tasks_name,
        value: ExprId::new(0),
        span: Span::new(0, 0),
    }];

    let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::list(vec![]));

    let ctx = make_ctx(&interner, &arena, &props);
    let result = SpawnPattern.evaluate(&ctx, &mut exec).unwrap();

    assert!(matches!(result, Value::Void));
}

#[test]
fn spawn_pattern_name() {
    assert_eq!(SpawnPattern.name(), "spawn");
}

#[test]
fn spawn_required_props() {
    assert_eq!(SpawnPattern.required_props(), &["tasks"]);
}

#[test]
fn spawn_does_not_allow_arbitrary_props() {
    assert!(!SpawnPattern.allows_arbitrary_props());
}

#[test]
fn spawn_requires_list_for_tasks() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let tasks_name = interner.intern("tasks");
    let props = vec![NamedExpr {
        name: tasks_name,
        value: ExprId::new(0),
        span: Span::new(0, 0),
    }];

    // Non-list value for tasks
    let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::int(42));

    let ctx = make_ctx(&interner, &arena, &props);
    let result = SpawnPattern.evaluate(&ctx, &mut exec);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .into_eval_error()
        .message
        .contains("must be a list"));
}
