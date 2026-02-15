//! Unreachable pattern implementation.

use crate::{EvalContext, EvalError, EvalResult, PatternDefinition, PatternExecutor};

#[cfg(test)]
use crate::Value;

/// The `unreachable` pattern halts execution indicating impossible code path.
///
/// Syntax: `unreachable()` or `unreachable(reason: expr)`
/// Type: `unreachable() -> Never` or `unreachable(reason: str) -> Never`
#[derive(Clone, Copy)]
pub struct UnreachablePattern;

impl PatternDefinition for UnreachablePattern {
    fn name(&self) -> &'static str {
        "unreachable"
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
            format!("entered unreachable code: {}", reason.display_value())
        } else {
            "entered unreachable code".to_string()
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
