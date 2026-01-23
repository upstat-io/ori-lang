//! Filter pattern implementation.
//!
//! `filter(.over: collection, .predicate: fn)` - Keep elements matching predicate.

use crate::types::Type;
use crate::eval::EvalResult;
use super::{
    PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor,
    FusedPattern, Iterable,
};

/// The `filter` pattern keeps elements that match a predicate.
///
/// Syntax: `filter(.over: items, .predicate: fn)`
///
/// Type: `filter(.over: [T], .predicate: T -> bool) -> [T]`
pub struct FilterPattern;

impl PatternDefinition for FilterPattern {
    fn name(&self) -> &'static str {
        "filter"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["over", "predicate"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // filter(.over: [T], .predicate: T -> bool) -> [T]
        let over_ty = ctx.require_prop_type("over");
        // Return type is same as input type (same list element type)
        over_ty
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let items = Iterable::try_from_value(ctx.eval_prop("over", exec)?)?;
        let func = ctx.eval_prop("predicate", exec)?;
        items.filter_values(&func, exec)
    }

    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        // filter can fuse with map, fold, or find
        matches!(next.name(), "map" | "fold" | "find")
    }

    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
        self_ctx: &EvalContext,
        next_ctx: &EvalContext,
    ) -> Option<FusedPattern> {
        // Get our input and predicate
        let input = self_ctx.get_prop("over").ok()?;
        let filter_fn = self_ctx.get_prop("predicate").ok()?;

        match next.name() {
            "map" => {
                let map_fn = next_ctx.get_prop("transform").ok()?;
                Some(FusedPattern::FilterMap {
                    input,
                    filter_fn,
                    map_fn,
                })
            }
            "fold" => {
                let init = next_ctx.get_prop("init").ok()?;
                let fold_fn = next_ctx.get_prop("op").ok()?;
                Some(FusedPattern::FilterFold {
                    input,
                    filter_fn,
                    init,
                    fold_fn,
                })
            }
            "find" => {
                let find_fn = next_ctx.get_prop("where").ok()?;
                Some(FusedPattern::FilterFind {
                    input,
                    filter_fn,
                    find_fn,
                })
            }
            _ => None,
        }
    }
}
