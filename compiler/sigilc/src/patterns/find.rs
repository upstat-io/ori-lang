//! Find pattern implementation.
//!
//! `find(over: collection, where: fn)` - Find first matching element.
//! `find(over: collection, map: fn)` - Find first Some from transformation (find_map).

use crate::types::Type;
use crate::eval::{Value, EvalResult};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor, Iterable};

/// The `find` pattern finds the first element matching a predicate.
///
/// Syntax: `find(over: items, where: fn)` or `find(over: items, map: fn)`
///
/// Regular find: `find(over: [T], where: T -> bool) -> Option<T>`
/// Find map: `find(over: [T], map: T -> Option<U>) -> Option<U>`
pub struct FindPattern;

impl PatternDefinition for FindPattern {
    fn name(&self) -> &'static str {
        "find"
    }

    fn required_props(&self) -> &'static [&'static str] {
        // Only .over is always required; .where or .map must be present
        &["over"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["where", "map", "default"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        let over_ty = ctx.require_prop_type("over");
        let has_map = ctx.get_prop_type("map").is_some();

        if has_map {
            // find_map: map: T -> Option<U>, returns Option<U>
            // The return type is Option<U> where U is the inner type of the map's return
            let fresh = ctx.fresh_var();
            ctx.option_of(fresh)
        } else {
            // Regular find: where: T -> bool, returns Option<T>
            match over_ty {
                Type::List(elem_ty) => ctx.option_of(*elem_ty),
                Type::Range(elem_ty) => ctx.option_of(*elem_ty),
                _ => {
                    let fresh = ctx.fresh_var();
                    ctx.option_of(fresh)
                }
            }
        }
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let items = Iterable::try_from_value(ctx.eval_prop("over", exec)?)?;
        let default_expr = ctx.get_prop_opt("default");

        // Check if we're using find_map (.map) or regular find (.where)
        let map_expr = ctx.get_prop_opt("map");

        if let Some(map_prop) = map_expr {
            // find_map: apply transformation, return first Some
            let func = exec.eval(map_prop)?;

            match items {
                Iterable::List(list) => {
                    for item in list.iter() {
                        let result = exec.call(func.clone(), vec![item.clone()])?;
                        match result {
                            Value::Some(inner) => return Ok(Value::Some(inner)),
                            Value::None => continue,
                            // If the function returns something other than Option, treat it as Some
                            other => return Ok(Value::some(other)),
                        }
                    }
                }
                Iterable::Range(range) => {
                    for i in range.iter() {
                        let result = exec.call(func.clone(), vec![Value::Int(i)])?;
                        match result {
                            Value::Some(inner) => return Ok(Value::Some(inner)),
                            Value::None => continue,
                            other => return Ok(Value::some(other)),
                        }
                    }
                }
            }

            // No Some found
            Ok(Value::None)
        } else {
            // Regular find with .where predicate
            let func = ctx.eval_prop("where", exec)?;

            match items.find_value(&func, exec)? {
                Some(value) => {
                    if default_expr.is_some() {
                        // With default, return the value directly
                        Ok(value)
                    } else {
                        // Without default, wrap in Some
                        Ok(Value::some(value))
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
    }

    // find is a terminal pattern - doesn't fuse with anything following it
}
