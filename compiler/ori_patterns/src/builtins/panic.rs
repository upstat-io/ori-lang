//! Panic pattern implementation.

use crate::{EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor};

#[cfg(test)]
use crate::Value;

/// The `panic` pattern halts execution with an error message.
///
/// Syntax: `panic(msg: expr)`
/// Type: `panic(msg: str) -> Never`
#[derive(Clone, Copy)]
pub struct PanicPattern;

impl PatternDefinition for PanicPattern {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["msg"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = ctx.eval_prop("msg", exec)?;
        Err(EvalError::new(format!("panic: {}", msg.display_value())).into())
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
