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
fn cache_non_function_returns_value_directly() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let op_name = interner.intern("operation");
    let props = vec![NamedExpr {
        name: op_name,
        value: ExprId::new(0),
        span: Span::new(0, 0),
    }];

    let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::int(42));

    let ctx = make_ctx(&interner, &arena, &props);
    let result = CachePattern.evaluate(&ctx, &mut exec).unwrap();

    assert_eq!(result, Value::int(42));
}

#[test]
fn cache_pattern_name() {
    assert_eq!(CachePattern.name(), "cache");
}

#[test]
fn cache_required_props() {
    assert_eq!(CachePattern.required_props(), &["operation"]);
}

#[test]
fn cache_optional_props() {
    assert_eq!(CachePattern.optional_props(), &["key", "ttl"]);
}
