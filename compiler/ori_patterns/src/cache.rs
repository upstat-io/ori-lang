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
mod tests;
