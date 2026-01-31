//! Todo pattern implementation.

use ori_types::Type;

use crate::{
    EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext,
};

#[cfg(test)]
use crate::{test_helpers::MockPatternExecutor, Value};

/// The `todo` pattern halts execution indicating unfinished code.
///
/// Syntax: `todo()` or `todo(reason: expr)`
/// Type: `todo() -> Never` or `todo(reason: str) -> Never`
pub struct TodoPattern;

impl PatternDefinition for TodoPattern {
    fn name(&self) -> &'static str {
        "todo"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &[] // reason is optional
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["reason"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Never
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = if let Some(reason_expr) = ctx.get_prop_opt("reason") {
            let reason = exec.eval(reason_expr)?;
            format!("not yet implemented: {}", reason.display_value())
        } else {
            "not yet implemented".to_string()
        };
        Err(EvalError::new(msg))
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
    fn todo_returns_error_without_reason() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let props = vec![];

        let mut exec = MockPatternExecutor::new();

        let ctx = make_ctx(&interner, &arena, &props);
        let result = TodoPattern.evaluate(&ctx, &mut exec);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("not yet implemented"));
    }

    #[test]
    fn todo_returns_error_with_reason() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let reason_name = interner.intern("reason");
        let props = vec![NamedExpr {
            name: reason_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        }];

        let mut exec = MockPatternExecutor::new()
            .with_expr(ExprId::new(0), Value::string("implement algorithm"));

        let ctx = make_ctx(&interner, &arena, &props);
        let result = TodoPattern.evaluate(&ctx, &mut exec);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("not yet implemented"));
        assert!(err.message.contains("implement algorithm"));
    }

    #[test]
    fn todo_pattern_name() {
        assert_eq!(TodoPattern.name(), "todo");
    }

    #[test]
    fn todo_required_props() {
        assert_eq!(TodoPattern.required_props(), &[] as &[&str]);
    }

    #[test]
    fn todo_optional_props() {
        assert_eq!(TodoPattern.optional_props(), &["reason"]);
    }

    #[test]
    fn todo_returns_never_type() {
        let interner = SharedInterner::default();
        let mut ctx = ori_types::InferenceContext::new();
        let mut type_ctx = TypeCheckContext::new(&interner, &mut ctx, Default::default());
        let result = TodoPattern.type_check(&mut type_ctx);
        assert!(matches!(result, Type::Never));
    }
}
