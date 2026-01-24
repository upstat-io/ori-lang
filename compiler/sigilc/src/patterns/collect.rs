//! Collect pattern implementation.
//!
//! `collect(range: range, transform: fn)` - Generate list from range.

use crate::types::Type;
use crate::eval::{Value, EvalResult, EvalError};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor, Iterable};

/// The `collect` pattern generates a list by transforming a range.
///
/// Syntax: `collect(range: 0..10, transform: fn)`
///
/// Type: `collect(range: Range<T>, transform: T -> U) -> [U]`
pub struct CollectPattern;

impl PatternDefinition for CollectPattern {
    fn name(&self) -> &'static str {
        "collect"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["range", "transform"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // collect(range: Range<T>, transform: T -> U) -> [U]
        let result_elem = ctx.get_function_return_type("transform");
        ctx.list_of(result_elem)
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let range_val = ctx.eval_prop("range", exec)?;
        let func = ctx.eval_prop("transform", exec)?;

        // Collect specifically requires a range, not a list
        if let Value::Range(_) = &range_val {
            let items = Iterable::try_from_value(range_val)?;
            items.map_values(&func, exec)
        } else {
            Err(EvalError::new("collect requires a range"))
        }
    }

}
