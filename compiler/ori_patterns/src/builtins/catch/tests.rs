use super::*;
use crate::test_helpers::{make_ctx, MockPatternExecutor};
use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

#[test]
fn catch_success_wraps_in_ok() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let expr_name = interner.intern("expr");
    let props = vec![NamedExpr {
        name: expr_name,
        value: ExprId::new(0),
        span: Span::new(0, 0),
    }];

    let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::int(42));

    let ctx = make_ctx(&interner, &arena, &props);
    let result = CatchPattern.evaluate(&ctx, &mut exec).unwrap();

    match result {
        Value::Ok(ref v) => assert_eq!(**v, Value::int(42)),
        _ => panic!("expected Ok variant"),
    }
}

#[test]
fn catch_error_wraps_in_err() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let expr_name = interner.intern("expr");
    let props = vec![NamedExpr {
        name: expr_name,
        value: ExprId::new(0),
        span: Span::new(0, 0),
    }];

    // MockPatternExecutor with no value for ExprId(0) will return an error
    let mut exec = MockPatternExecutor::new();

    let ctx = make_ctx(&interner, &arena, &props);
    let result = CatchPattern.evaluate(&ctx, &mut exec).unwrap();

    // Should be Err containing the error message
    assert!(matches!(result, Value::Err(_)));
}

#[test]
fn catch_pattern_name() {
    assert_eq!(CatchPattern.name(), "catch");
}

#[test]
fn catch_required_props() {
    assert_eq!(CatchPattern.required_props(), &["expr"]);
}
