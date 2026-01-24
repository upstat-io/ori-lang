//! Fold pattern implementation.
//!
//! `fold(over: collection, initial: value, operation: fn)` - Reduce to single value.

use crate::types::Type;
use crate::eval::EvalResult;
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor, Iterable};

/// The `fold` pattern reduces a collection to a single value.
///
/// Syntax: `fold(over: items, initial: value, operation: fn)`
///
/// Type: `fold(over: [T], initial: U, operation: (U, T) -> U) -> U`
pub struct FoldPattern;

impl PatternDefinition for FoldPattern {
    fn name(&self) -> &'static str {
        "fold"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["over", "initial", "operation"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // fold(over: [T], initial: U, operation: (U, T) -> U) -> U
        let init_ty = ctx.get_prop_type("initial").unwrap_or_else(|| ctx.fresh_var());
        init_ty
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let items = Iterable::try_from_value(ctx.eval_prop("over", exec)?)?;
        let acc = ctx.eval_prop("initial", exec)?;
        let func = ctx.eval_prop("operation", exec)?;
        items.fold_values(acc, &func, exec)
    }

    // fold is a terminal pattern - it doesn't fuse with anything following it
    // (can_fuse_with defaults to false)
}
