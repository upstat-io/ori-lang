//! Todo pattern implementation.

use crate::{EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor};

#[cfg(test)]
use crate::Value;

/// The `todo` pattern halts execution indicating unfinished code.
///
/// Syntax: `todo()` or `todo(reason: expr)`
/// Type: `todo() -> Never` or `todo(reason: str) -> Never`
#[derive(Clone, Copy)]
pub struct TodoPattern;

impl PatternDefinition for TodoPattern {
    fn name(&self) -> &'static str {
        "todo"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &[] // reason is optional
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["reason"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = if let Some(reason_expr) = ctx.get_prop_opt("reason") {
            let reason = exec.eval(reason_expr)?;
            format!("not yet implemented: {}", reason.display_value())
        } else {
            "not yet implemented".to_string()
        };
        Err(EvalError::new(msg).into())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests use unwrap to panic on unexpected state"
)]
#[allow(
    clippy::default_trait_access,
    reason = "SharedInterner::default() is clearer than import"
)]
mod tests;
