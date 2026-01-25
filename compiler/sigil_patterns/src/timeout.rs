//! Timeout pattern implementation.
//!
//! `timeout(operation: expr, after: duration)` - Execute with timeout.

use sigil_types::Type;

use crate::{
    EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext, Value,
};

/// The `timeout` pattern executes an operation with a timeout.
///
/// Syntax: `timeout(operation: expr, after: 5s)`
///
/// Type: `timeout(operation: T, after: Duration) -> Result<T, TimeoutError>`
///
/// Note: In the interpreter, timeout is not enforced. Actual timeout
/// behavior is implemented in the compiled output.
pub struct TimeoutPattern;

impl PatternDefinition for TimeoutPattern {
    fn name(&self) -> &'static str {
        "timeout"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["operation", "after"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // timeout(operation: T, after: Duration) -> Result<T, Error>
        let op_ty = ctx
            .get_prop_type("operation")
            .unwrap_or_else(|| ctx.fresh_var());
        ctx.result_of(op_ty, Type::Error)
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
