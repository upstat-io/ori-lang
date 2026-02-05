//! Timeout pattern implementation.
//!
//! `timeout(operation: expr, after: duration)` - Execute with timeout.

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, Value};

#[cfg(test)]
use crate::test_helpers::MockPatternExecutor;

/// The `timeout` pattern executes an operation with a timeout.
///
/// Syntax: `timeout(operation: expr, after: 5s)`
///
/// Type: `timeout(operation: T, after: Duration) -> Result<T, TimeoutError>`
///
/// Note: In the interpreter, timeout is not enforced. Actual timeout
/// behavior is implemented in the compiled output.
#[derive(Clone, Copy)]
pub struct TimeoutPattern;

impl PatternDefinition for TimeoutPattern {
    fn name(&self) -> &'static str {
        "timeout"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["operation", "after"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        // Validate that .after is present (for type checking), but don't use it
        let _ = ctx.get_prop("after")?;

        // In interpreter, just run the operation without actual timeout
        match ctx.eval_prop("operation", exec) {
            Ok(value) => Ok(Value::ok(value)),
            Err(e) => Ok(Value::err(Value::string(e.message))),
        }
    }
}

#[cfg(test)]
// Tests use unwrap() to panic on unexpected state, making failures immediately visible
#[allow(clippy::unwrap_used)]
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
    fn timeout_success_wraps_in_ok() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let op_name = interner.intern("operation");
        let after_name = interner.intern("after");
        let props = vec![
            NamedExpr {
                name: op_name,
                value: ExprId::new(0),
                span: Span::new(0, 0),
            },
            NamedExpr {
                name: after_name,
                value: ExprId::new(1),
                span: Span::new(0, 0),
            },
        ];

        let mut exec = MockPatternExecutor::new()
            .with_expr(ExprId::new(0), Value::int(42))
            .with_expr(ExprId::new(1), Value::int(5000)); // 5s as int

        let ctx = make_ctx(&interner, &arena, &props);
        let result = TimeoutPattern.evaluate(&ctx, &mut exec).unwrap();

        match result {
            Value::Ok(ref v) => assert_eq!(**v, Value::int(42)),
            _ => panic!("expected Ok variant"),
        }
    }

    #[test]
    fn timeout_error_wraps_in_err() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let op_name = interner.intern("operation");
        let after_name = interner.intern("after");
        let props = vec![
            NamedExpr {
                name: op_name,
                value: ExprId::new(0),
                span: Span::new(0, 0),
            },
            NamedExpr {
                name: after_name,
                value: ExprId::new(1),
                span: Span::new(0, 0),
            },
        ];

        // Only after has a value, operation will error
        let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(1), Value::int(5000));

        let ctx = make_ctx(&interner, &arena, &props);
        let result = TimeoutPattern.evaluate(&ctx, &mut exec).unwrap();

        assert!(matches!(result, Value::Err(_)));
    }

    #[test]
    fn timeout_pattern_name() {
        assert_eq!(TimeoutPattern.name(), "timeout");
    }

    #[test]
    fn timeout_required_props() {
        assert_eq!(TimeoutPattern.required_props(), &["operation", "after"]);
    }
}
