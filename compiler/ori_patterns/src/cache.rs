//! Cache pattern implementation.
//!
//! `cache(operation: fn)` - Memoize computation result.

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, Value};

#[cfg(test)]
use crate::test_helpers::MockPatternExecutor;

/// The `cache` pattern memoizes computation results.
///
/// Syntax: `cache(operation: fn)`
///
/// Optional: `key: value`, `ttl: duration`
///
/// Type: `cache(operation: () -> T) -> T`
///
/// Note: In the interpreter, caching is not implemented. The operation
/// function is called each time. Actual caching is implemented in compiled output.
#[derive(Clone, Copy)]
pub struct CachePattern;

impl PatternDefinition for CachePattern {
    fn name(&self) -> &'static str {
        "cache"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["operation"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["key", "ttl"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let func = ctx.eval_prop("operation", exec)?;

        // Call the compute function with no arguments
        match func {
            Value::Function(_) | Value::FunctionVal(_, _) => exec.call(&func, vec![]),
            _ => {
                // If not a function, just return the value
                Ok(func)
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests use unwrap to panic on unexpected state"
)]
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
}
