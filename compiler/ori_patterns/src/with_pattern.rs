//! With pattern implementation.
//!
//! `with(acquire: expr, action: fn, release: fn)` - Resource management.
//!
//! The property is named `action` rather than `use` because `use` is a reserved keyword.

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor};

#[cfg(test)]
use crate::{test_helpers::MockPatternExecutor, Value};

/// The `with` pattern provides structured resource management.
///
/// Syntax: `with(acquire: resource, action: r -> expr, release: r -> void)`
///
/// Type: `with(acquire: R, action: R -> T, release: R -> void) -> T`
///
/// The property is named `action` rather than `use` because `use` is a reserved keyword.
/// The release function is always called, even if action throws.
#[derive(Clone, Copy)]
pub struct WithPattern;

impl PatternDefinition for WithPattern {
    fn name(&self) -> &'static str {
        "with"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["acquire", "action"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["release"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let release_expr = ctx.get_prop_opt("release");

        let resource = ctx.eval_prop("acquire", exec)?;
        let action_fn = ctx.eval_prop("action", exec)?;

        // Call action function with resource
        let result = exec.call(&action_fn, vec![resource.clone()]);

        // Always call release if provided (RAII pattern)
        if let Some(rel_expr) = release_expr {
            let release_fn = exec.eval(rel_expr)?;
            let _ = exec.call(&release_fn, vec![resource]);
        }

        result
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests use unwrap to panic on unexpected state"
)]
mod tests;
