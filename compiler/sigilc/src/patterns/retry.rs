//! Retry pattern implementation.
//!
//! `retry(operation: expr, attempts: n)` - Retry failed operations.

use crate::types::Type;
use crate::eval::{EvalResult, EvalError};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

/// The `retry` pattern retries failed operations.
///
/// Syntax: `retry(operation: expr, attempts: 3)`
///
/// Optional: `backoff: strategy` for exponential backoff
///
/// Type: `retry(operation: T, attempts: int) -> T`
pub struct RetryPattern;

impl PatternDefinition for RetryPattern {
    fn name(&self) -> &'static str {
        "retry"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["operation", "attempts"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["backoff"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // retry(operation: T, attempts: int) -> T
        let op_ty = ctx.get_prop_type("operation").unwrap_or_else(|| ctx.fresh_var());
        op_ty
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let op_expr = ctx.get_prop("operation")?;
        let attempts = ctx.eval_prop("attempts", exec)?
            .as_int()
            .ok_or_else(|| EvalError::new("retry .attempts must be an integer"))?;

        let mut last_error = None;
        for _ in 0..attempts {
            match exec.eval(op_expr) {
                Ok(value) => return Ok(value),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.unwrap_or_else(|| EvalError::new("retry failed")))
    }

}
