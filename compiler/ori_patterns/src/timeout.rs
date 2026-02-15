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
