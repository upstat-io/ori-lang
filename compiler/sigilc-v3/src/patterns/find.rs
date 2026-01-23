//! Find pattern implementation.
//!
//! `find(.over: collection, .where: fn)` - Find first matching element.

use crate::types::Type;
use crate::eval::{Value, EvalResult};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor, Iterable};

/// The `find` pattern finds the first element matching a predicate.
///
/// Syntax: `find(.over: items, .where: fn)`
///
/// Type: `find(.over: [T], .where: T -> bool) -> Option<T>`
pub struct FindPattern;

impl PatternDefinition for FindPattern {
    fn name(&self) -> &'static str {
        "find"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["over", "where"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["default"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // find(.over: [T], .where: T -> bool) -> Option<T>
        let over_ty = ctx.require_prop_type("over");
        match over_ty {
            Type::List(elem_ty) => ctx.option_of(*elem_ty),
            Type::Range(elem_ty) => ctx.option_of(*elem_ty),
            _ => {
                let fresh = ctx.fresh_var();
                ctx.option_of(fresh)
            }
        }
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let items = Iterable::try_from_value(ctx.eval_prop("over", exec)?)?;
        let func = ctx.eval_prop("where", exec)?;
        let default_expr = ctx.get_prop_opt("default");

        match items.find_value(&func, exec)? {
            Some(value) => {
                if default_expr.is_some() {
                    // With default, return the value directly
                    Ok(value)
                } else {
                    // Without default, wrap in Some
                    Ok(Value::Some(Box::new(value)))
                }
            }
            None => {
                if let Some(def) = default_expr {
                    exec.eval(def)
                } else {
                    Ok(Value::None)
                }
            }
        }
    }

    // find is a terminal pattern - doesn't fuse with anything following it
}
