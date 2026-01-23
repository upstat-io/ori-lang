//! Map pattern implementation.
//!
//! `map(.over: collection, .transform: fn)` - Transform each element.

use crate::ir::TypeId;
use crate::types::Type;
use crate::eval::EvalResult;
use super::{
    PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor,
    PatternSignature, FusedPattern, Iterable,
};

/// The `map` pattern transforms each element of a collection.
///
/// Syntax: `map(.over: items, .transform: fn)`
///
/// Type: `map(.over: [T], .transform: T -> U) -> [U]`
pub struct MapPattern;

impl PatternDefinition for MapPattern {
    fn name(&self) -> &'static str {
        "map"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["over", "transform"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        let over_ty = ctx.require_prop_type("over");
        let result_elem = ctx.get_function_return_type("transform");

        // map works on both lists and ranges, always returns a list
        match over_ty {
            Type::List(_) | Type::Range(_) => ctx.list_of(result_elem),
            _ => ctx.fresh_var(),
        }
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let items = Iterable::try_from_value(ctx.eval_prop("over", exec)?)?;
        let func = ctx.eval_prop("transform", exec)?;
        items.map_values(&func, exec)
    }

    fn signature(&self, ctx: &TypeCheckContext) -> PatternSignature {
        let kind = ctx.interner.intern(self.name());
        // Get the result type from type checking context
        let output_type = match ctx.get_prop_type("transform") {
            Some(Type::Function { ret, .. }) => TypeId::INFER, // Would need type interning
            _ => TypeId::INFER,
        };
        PatternSignature::new(kind, output_type)
    }

    fn can_fuse_with(&self, next: &dyn PatternDefinition) -> bool {
        // map can fuse with filter, fold, or find
        matches!(next.name(), "filter" | "fold" | "find")
    }

    fn fuse_with(
        &self,
        next: &dyn PatternDefinition,
        self_ctx: &EvalContext,
        next_ctx: &EvalContext,
    ) -> Option<FusedPattern> {
        // Get our input and transform
        let input = self_ctx.get_prop("over").ok()?;
        let map_fn = self_ctx.get_prop("transform").ok()?;

        match next.name() {
            "filter" => {
                let filter_fn = next_ctx.get_prop("predicate").ok()?;
                Some(FusedPattern::MapFilter {
                    input,
                    map_fn,
                    filter_fn,
                })
            }
            "fold" => {
                let init = next_ctx.get_prop("init").ok()?;
                let fold_fn = next_ctx.get_prop("op").ok()?;
                Some(FusedPattern::MapFold {
                    input,
                    map_fn,
                    init,
                    fold_fn,
                })
            }
            "find" => {
                let find_fn = next_ctx.get_prop("where").ok()?;
                Some(FusedPattern::MapFind {
                    input,
                    map_fn,
                    find_fn,
                })
            }
            _ => None,
        }
    }
}
