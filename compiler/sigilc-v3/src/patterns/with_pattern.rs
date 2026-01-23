//! With pattern implementation.
//!
//! `with(.acquire: expr, .use: fn, .release: fn)` - Resource management.

use crate::types::Type;
use crate::eval::EvalResult;
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

/// The `with` pattern provides structured resource management.
///
/// Syntax: `with(.acquire: resource, .use: r -> expr, .release: r -> void)`
///
/// Type: `with(.acquire: R, .use: R -> T, .release: R -> void) -> T`
///
/// The release function is always called, even if use throws.
pub struct WithPattern;

impl PatternDefinition for WithPattern {
    fn name(&self) -> &'static str {
        "with"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["acquire", "use"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["release"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // with(.acquire: R, .use: R -> T, .release: R -> void) -> T
        ctx.get_function_return_type("use")
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let release_expr = ctx.get_prop_opt("release");

        let resource = ctx.eval_prop("acquire", exec)?;
        let use_fn = ctx.eval_prop("use", exec)?;

        // Call use function with resource
        let result = exec.call(use_fn, vec![resource.clone()]);

        // Always call release if provided (RAII pattern)
        if let Some(rel_expr) = release_expr {
            let release_fn = exec.eval(rel_expr)?;
            let _ = exec.call(release_fn, vec![resource]);
        }

        result
    }

}
