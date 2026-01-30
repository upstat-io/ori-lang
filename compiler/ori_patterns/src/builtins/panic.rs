//! Panic pattern implementation.

use ori_types::Type;

use crate::{
    EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext,
};

#[cfg(test)]
use crate::{test_helpers::MockPatternExecutor, Value};

/// The `panic` pattern halts execution with an error message.
///
/// Syntax: `panic(msg: expr)`
/// Type: `panic(msg: str) -> Never`
pub struct PanicPattern;

impl PatternDefinition for PanicPattern {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["msg"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Never
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = ctx.eval_prop("msg", exec)?;
        Err(EvalError::new(format!("panic: {}", msg.display_value())))
    }
}

#[cfg(test)]
// Tests use unwrap() to panic on unexpected state, making failures immediately visible
#[allow(clippy::unwrap_used, clippy::default_trait_access)]
mod tests {
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
        let err = result.unwrap_err();
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

    #[test]
    fn panic_returns_never_type() {
        let interner = SharedInterner::default();
        let mut ctx = ori_types::InferenceContext::new();
        let mut type_ctx = TypeCheckContext::new(&interner, &mut ctx, Default::default());
        let result = PanicPattern.type_check(&mut type_ctx);
        assert!(matches!(result, Type::Never));
    }
}
