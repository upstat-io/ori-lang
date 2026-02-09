//! Unreachable pattern implementation.

use crate::{EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor};

#[cfg(test)]
use crate::Value;

/// The `unreachable` pattern halts execution indicating impossible code path.
///
/// Syntax: `unreachable()` or `unreachable(reason: expr)`
/// Type: `unreachable() -> Never` or `unreachable(reason: str) -> Never`
#[derive(Clone, Copy)]
pub struct UnreachablePattern;

impl PatternDefinition for UnreachablePattern {
    fn name(&self) -> &'static str {
        "unreachable"
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
            format!("entered unreachable code: {}", reason.display_value())
        } else {
            "entered unreachable code".to_string()
        };
        Err(EvalError::new(msg).into())
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
}
