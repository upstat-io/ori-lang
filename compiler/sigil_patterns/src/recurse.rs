//! Recurse pattern implementation.
//!
//! `recurse(condition: bool, base: value, step: expr)` - Conditional recursion.

use sigil_types::Type;

use crate::{
    DefaultValue, EvalContext, EvalResult, OptionalArg, PatternDefinition, PatternExecutor,
    TypeCheckContext,
};

/// The `recurse` pattern enables conditional recursion with optional memoization.
///
/// Syntax: `recurse(condition: base_case, base: value, step: self(...))`
///
/// Optional:
/// - `memo: true` for memoization
///
/// Type: `recurse(condition: bool, base: T, step: T) -> T`
pub struct RecursePattern;

impl PatternDefinition for RecursePattern {
    fn name(&self) -> &'static str {
        "recurse"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["condition", "base", "step"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["memo"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // recurse(cond: bool, base: T, step: T) -> T
        
        ctx.get_prop_type("base").unwrap_or_else(|| ctx.fresh_var())
    }

    fn optional_args(&self) -> &'static [OptionalArg] {
        static OPTIONAL: [OptionalArg; 1] = [
            OptionalArg {
                name: "memo",
                default: DefaultValue::Bool(false),
            },
        ];
        &OPTIONAL
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let base_expr = ctx.get_prop("base")?;
        let step_expr = ctx.get_prop("step")?;

        let cond_val = ctx.eval_prop("condition", exec)?;

        if cond_val.is_truthy() {
            exec.eval(base_expr)
        } else {
            exec.eval(step_expr)
        }
    }
}
