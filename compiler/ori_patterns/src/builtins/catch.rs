//! Catch pattern implementation.

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, Value};

/// The `catch` pattern captures panics and converts them to `Result<T, str>`.
///
/// Syntax: `catch(expr: expression)`
/// Type: `catch(expr: T) -> Result<T, str>`
#[derive(Clone, Copy)]
pub struct CatchPattern;

impl PatternDefinition for CatchPattern {
    fn name(&self) -> &'static str {
        "catch"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["expr"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        match ctx.eval_prop("expr", exec) {
            Ok(value) => Ok(Value::ok(value)),
            Err(e) => Ok(Value::err(Value::string(e.into_eval_error().message))),
        }
    }
}

#[cfg(test)]
// Tests use unwrap() to panic on unexpected state, making failures immediately visible
#[allow(clippy::unwrap_used)]
mod tests {
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
}
