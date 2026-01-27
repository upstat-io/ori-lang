//! Recurse pattern implementation.
//!
//! `recurse(condition: bool, base: value, step: expr)` - Conditional recursion.

use ori_types::Type;

use crate::{
    DefaultValue, EvalContext, EvalResult, MemoizedFunctionValue, OptionalArg, PatternDefinition,
    PatternExecutor, ScopedBinding, ScopedBindingType, TypeCheckContext, Value,
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

    fn scoped_bindings(&self) -> &'static [ScopedBinding] {
        // `self` is a function with the same signature as the enclosing function.
        // This enables recursive calls like `self(n - 1)` in the `step` property.
        static BINDINGS: [ScopedBinding; 1] = [
            ScopedBinding {
                name: "self",
                for_props: &["step"],
                type_from: ScopedBindingType::EnclosingFunction,
            },
        ];
        &BINDINGS
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let base_expr = ctx.get_prop("base")?;
        let step_expr = ctx.get_prop("step")?;

        // Check if memoization is enabled
        let memo_enabled = ctx
            .eval_prop_opt("memo", exec)?
            .is_some_and(|v| v.is_truthy());

        // If memo is enabled, wrap `self` in a memoized function
        if memo_enabled {
            if let Some(Value::Function(f)) = exec.lookup_var("self") {
                // Create memoized wrapper and rebind `self`
                let memoized = Value::MemoizedFunction(MemoizedFunctionValue::new(f));
                exec.bind_var("self", memoized);
            }
        }

        let cond_val = ctx.eval_prop("condition", exec)?;

        if cond_val.is_truthy() {
            exec.eval(base_expr)
        } else {
            exec.eval(step_expr)
        }
    }
}
