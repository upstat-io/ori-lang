//! Todo pattern implementation.

use crate::{EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor};

#[cfg(test)]
use crate::Value;

/// The `todo` pattern halts execution indicating unfinished code.
///
/// Syntax: `todo()` or `todo(reason: expr)`
/// Type: `todo() -> Never` or `todo(reason: str) -> Never`
#[derive(Clone, Copy)]
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
    use crate::test_helpers::{make_ctx, MockPatternExecutor};
    use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

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
}
