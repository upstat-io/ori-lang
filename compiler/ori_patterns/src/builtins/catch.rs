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
#[allow(
    clippy::unwrap_used,
    reason = "tests use unwrap to panic on unexpected state"
)]
mod tests;
