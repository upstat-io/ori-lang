use super::*;
use crate::test_helpers::{make_ctx, MockPatternExecutor};
use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

#[test]
fn unreachable_returns_error_without_reason() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let props = vec![];

    let mut exec = MockPatternExecutor::new();

    let ctx = make_ctx(&interner, &arena, &props);
    let result = UnreachablePattern.evaluate(&ctx, &mut exec);

    assert!(result.is_err());
    let err = result.unwrap_err().into_eval_error();
    assert!(err.message.contains("entered unreachable code"));
}

#[test]
fn unreachable_returns_error_with_reason() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let reason_name = interner.intern("reason");
    let props = vec![NamedExpr {
        name: reason_name,
        value: ExprId::new(0),
        span: Span::new(0, 0),
    }];

    let mut exec =
        MockPatternExecutor::new().with_expr(ExprId::new(0), Value::string("impossible state"));

    let ctx = make_ctx(&interner, &arena, &props);
    let result = UnreachablePattern.evaluate(&ctx, &mut exec);

    assert!(result.is_err());
    let err = result.unwrap_err().into_eval_error();
    assert!(err.message.contains("entered unreachable code"));
    assert!(err.message.contains("impossible state"));
}

#[test]
fn unreachable_pattern_name() {
    assert_eq!(UnreachablePattern.name(), "unreachable");
}

#[test]
fn unreachable_required_props() {
    assert_eq!(UnreachablePattern.required_props(), &[] as &[&str]);
}

#[test]
fn unreachable_optional_props() {
    assert_eq!(UnreachablePattern.optional_props(), &["reason"]);
}
