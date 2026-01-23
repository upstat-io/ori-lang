//! Cache pattern implementation.
//!
//! `cache(.compute: fn)` - Memoize computation result.

use crate::types::Type;
use crate::eval::{Value, EvalResult};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

/// The `cache` pattern memoizes computation results.
///
/// Syntax: `cache(.compute: fn)`
///
/// Optional: `.key: value`, `.ttl: duration`
///
/// Type: `cache(.compute: () -> T) -> T`
///
/// Note: In the interpreter, caching is not implemented. The compute
/// function is called each time. Actual caching is implemented in compiled output.
pub struct CachePattern;

impl PatternDefinition for CachePattern {
    fn name(&self) -> &'static str {
        "cache"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["compute"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["key", "ttl"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // cache(.compute: () -> T) -> T
        // Or cache(.compute: fn) where fn is called to get T
        let compute_ty = ctx.require_prop_type("compute");
        match compute_ty {
            Type::Function { ret, .. } => *ret,
            // If compute is not a function, return its type directly
            other => other,
        }
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let func = ctx.eval_prop("compute", exec)?;

        // Call the compute function with no arguments
        match func {
            Value::Function(_) | Value::FunctionVal(_, _) => {
                exec.call(func, vec![])
            }
            _ => {
                // If not a function, just return the value
                Ok(func)
            }
        }
    }

}
