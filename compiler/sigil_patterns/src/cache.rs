//! Cache pattern implementation.
//!
//! `cache(operation: fn)` - Memoize computation result.

use sigil_types::Type;

use crate::{
    EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext, Value,
};

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
            Value::Function(_) | Value::FunctionVal(_, _) => exec.call(func, vec![]),
            _ => {
                // If not a function, just return the value
                Ok(func)
            }
        }
    }
}
