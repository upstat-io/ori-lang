use super::*;
use crate::test_helpers::{make_ctx, MockPatternExecutor};
use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

#[test]
fn panic_returns_error_with_message() {
    let interner = SharedInterner::default();
    let arena = ExprArena::new();
    let msg_name = interner.intern("msg");
    let props = vec![NamedExpr {
        name: msg_name,
        value: ExprId::new(0),
        span: Span::new(0, 0),
    }];

    let mut exec =
        MockPatternExecutor::new().with_expr(ExprId::new(0), Value::string("test error"));

    let ctx = make_ctx(&interner, &arena, &props);
    let result = PanicPattern.evaluate(&ctx, &mut exec);

    assert!(result.is_err());
    let action = result.unwrap_err();
    let err = action.into_eval_error();
    assert!(err.message.contains("panic:"));
    assert!(err.message.contains("test error"));
}

#[test]
fn panic_pattern_name() {
    assert_eq!(PanicPattern.name(), "panic");
}

#[test]
fn panic_required_props() {
    assert_eq!(PanicPattern.required_props(), &["msg"]);
}
