//! Cache pattern implementation.
//!
//! `cache(operation: fn)` - Memoize computation result.

use ori_types::Type;

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext, Value};

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

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // cache(operation: () -> T) -> T
        // Or cache(operation: fn) where fn is called to get T
        let compute_ty = ctx.require_prop_type("operation");
        match compute_ty {
            Type::Function { ret, .. } => *ret,
            // If operation is not a function, return its type directly
            other => other,
        }
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

    #[test]
    fn cache_extracts_function_return_type() {
        let interner = SharedInterner::default();
        let mut ctx = ori_types::InferenceContext::new();

        let mut prop_types = rustc_hash::FxHashMap::default();
        let op_name = interner.intern("operation");
        prop_types.insert(
            op_name,
            Type::Function {
                params: vec![],
                ret: Box::new(Type::Int),
            },
        );

        let mut type_ctx = TypeCheckContext::new(&interner, &mut ctx, prop_types);
        let result = CachePattern.type_check(&mut type_ctx);

        assert!(matches!(result, Type::Int));
    }

    #[test]
    fn cache_non_function_type_returns_as_is() {
        let interner = SharedInterner::default();
        let mut ctx = ori_types::InferenceContext::new();

        let mut prop_types = rustc_hash::FxHashMap::default();
        let op_name = interner.intern("operation");
        prop_types.insert(op_name, Type::Str);

        let mut type_ctx = TypeCheckContext::new(&interner, &mut ctx, prop_types);
        let result = CachePattern.type_check(&mut type_ctx);

        assert!(matches!(result, Type::Str));
    }
}
