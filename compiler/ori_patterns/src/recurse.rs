//! Recurse pattern implementation.
//!
//! `recurse(condition: bool, base: value, step: expr)` - Conditional recursion.

use crate::{
    DefaultValue, EvalContext, EvalResult, MemoizedFunctionValue, OptionalArg, PatternDefinition,
    PatternExecutor, ScopedBinding, ScopedBindingType, Value,
};

#[cfg(test)]
use crate::test_helpers::MockPatternExecutor;

/// The `recurse` pattern enables conditional recursion with optional memoization.
///
/// Syntax: `recurse(condition: base_case, base: value, step: self(...))`
///
/// Optional:
/// - `memo: true` for memoization
///
/// Type: `recurse(condition: bool, base: T, step: T) -> T`
#[derive(Clone, Copy)]
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

    fn optional_args(&self) -> &'static [OptionalArg] {
        static OPTIONAL: [OptionalArg; 1] = [OptionalArg {
            name: "memo",
            default: DefaultValue::Bool(false),
        }];
        &OPTIONAL
    }

    fn scoped_bindings(&self) -> &'static [ScopedBinding] {
        // `self` is a function with the same signature as the enclosing function.
        // This enables recursive calls like `self(n - 1)` in the `step` property.
        static BINDINGS: [ScopedBinding; 1] = [ScopedBinding {
            name: "self",
            for_props: &["step"],
            type_from: ScopedBindingType::EnclosingFunction,
        }];
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
        let self_name = ctx.interner.intern("self");
        if memo_enabled {
            if let Some(Value::Function(f)) = exec.lookup_var(self_name) {
                // Create memoized wrapper and rebind `self`
                let memoized = Value::MemoizedFunction(MemoizedFunctionValue::new(f));
                exec.bind_var(self_name, memoized);
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests use unwrap to panic on unexpected state"
)]
mod tests {
    use super::*;
    use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

    fn make_ctx<'a>(
        interner: &'a SharedInterner,
        arena: &'a ExprArena,
        props: &'a [NamedExpr],
    ) -> EvalContext<'a> {
        EvalContext::new(interner, arena, props)
    }

    #[test]
    fn recurse_returns_base_when_condition_true() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let cond_name = interner.intern("condition");
        let base_name = interner.intern("base");
        let step_name = interner.intern("step");
        let props = vec![
            NamedExpr {
                name: cond_name,
                value: ExprId::new(0),
                span: Span::new(0, 0),
            },
            NamedExpr {
                name: base_name,
                value: ExprId::new(1),
                span: Span::new(0, 0),
            },
            NamedExpr {
                name: step_name,
                value: ExprId::new(2),
                span: Span::new(0, 0),
            },
        ];

        let mut exec = MockPatternExecutor::new()
            .with_expr(ExprId::new(0), Value::Bool(true)) // condition is true
            .with_expr(ExprId::new(1), Value::int(42)) // base value
            .with_expr(ExprId::new(2), Value::int(100)); // step value

        let ctx = make_ctx(&interner, &arena, &props);
        let result = RecursePattern.evaluate(&ctx, &mut exec).unwrap();

        assert_eq!(result, Value::int(42));
    }

    #[test]
    fn recurse_returns_step_when_condition_false() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let cond_name = interner.intern("condition");
        let base_name = interner.intern("base");
        let step_name = interner.intern("step");
        let props = vec![
            NamedExpr {
                name: cond_name,
                value: ExprId::new(0),
                span: Span::new(0, 0),
            },
            NamedExpr {
                name: base_name,
                value: ExprId::new(1),
                span: Span::new(0, 0),
            },
            NamedExpr {
                name: step_name,
                value: ExprId::new(2),
                span: Span::new(0, 0),
            },
        ];

        let mut exec = MockPatternExecutor::new()
            .with_expr(ExprId::new(0), Value::Bool(false)) // condition is false
            .with_expr(ExprId::new(1), Value::int(42)) // base value
            .with_expr(ExprId::new(2), Value::int(100)); // step value

        let ctx = make_ctx(&interner, &arena, &props);
        let result = RecursePattern.evaluate(&ctx, &mut exec).unwrap();

        assert_eq!(result, Value::int(100));
    }

    #[test]
    fn recurse_pattern_name() {
        assert_eq!(RecursePattern.name(), "recurse");
    }

    #[test]
    fn recurse_required_props() {
        assert_eq!(
            RecursePattern.required_props(),
            &["condition", "base", "step"]
        );
    }

    #[test]
    fn recurse_optional_props() {
        assert_eq!(RecursePattern.optional_props(), &["memo"]);
    }

    #[test]
    fn recurse_has_scoped_bindings_for_self() {
        let bindings = RecursePattern.scoped_bindings();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].name, "self");
        assert_eq!(bindings[0].for_props, &["step"]);
    }
}
