//! With pattern implementation.
//!
//! `with(acquire: expr, action: fn, release: fn)` - Resource management.
//!
//! The property is named `action` rather than `use` because `use` is a reserved keyword.

use ori_types::Type;

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext};

#[cfg(test)]
use crate::{test_helpers::MockPatternExecutor, Value};

/// The `with` pattern provides structured resource management.
///
/// Syntax: `with(acquire: resource, action: r -> expr, release: r -> void)`
///
/// Type: `with(acquire: R, action: R -> T, release: R -> void) -> T`
///
/// The property is named `action` rather than `use` because `use` is a reserved keyword.
/// The release function is always called, even if action throws.
pub struct WithPattern;

impl PatternDefinition for WithPattern {
    fn name(&self) -> &'static str {
        "with"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["acquire", "action"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["release"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // with(acquire: R, action: R -> T, release: R -> void) -> T
        ctx.get_function_return_type("action")
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let release_expr = ctx.get_prop_opt("release");

        let resource = ctx.eval_prop("acquire", exec)?;
        let action_fn = ctx.eval_prop("action", exec)?;

        // Call action function with resource
        let result = exec.call(&action_fn, vec![resource.clone()]);

        // Always call release if provided (RAII pattern)
        if let Some(rel_expr) = release_expr {
            let release_fn = exec.eval(rel_expr)?;
            let _ = exec.call(&release_fn, vec![resource]);
        }

        result
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
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
    fn with_pattern_name() {
        assert_eq!(WithPattern.name(), "with");
    }

    #[test]
    fn with_required_props() {
        assert_eq!(WithPattern.required_props(), &["acquire", "action"]);
    }

    #[test]
    fn with_optional_props() {
        assert_eq!(WithPattern.optional_props(), &["release"]);
    }

    #[test]
    fn with_returns_action_result() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let acquire_name = interner.intern("acquire");
        let action_name = interner.intern("action");
        let props = vec![
            NamedExpr {
                name: acquire_name,
                value: ExprId::new(0),
                span: Span::new(0, 0),
            },
            NamedExpr {
                name: action_name,
                value: ExprId::new(1),
                span: Span::new(0, 0),
            },
        ];

        // Mock returns resource for acquire, action function, and call result
        let mut exec = MockPatternExecutor::new()
            .with_expr(ExprId::new(0), Value::string("resource"))
            .with_expr(ExprId::new(1), Value::Void) // Action function placeholder
            .with_call_results(vec![Value::int(42)]); // Action call returns 42

        let ctx = make_ctx(&interner, &arena, &props);
        let result = WithPattern.evaluate(&ctx, &mut exec).unwrap();

        assert_eq!(result, Value::int(42));
    }

    #[test]
    fn with_extracts_action_return_type() {
        let interner = SharedInterner::default();
        let mut ctx = ori_types::InferenceContext::new();

        let mut prop_types = std::collections::HashMap::new();
        let action_name = interner.intern("action");
        prop_types.insert(
            action_name,
            Type::Function {
                params: vec![Type::Str],
                ret: Box::new(Type::Int),
            },
        );

        let mut type_ctx = TypeCheckContext::new(&interner, &mut ctx, prop_types);
        let result = WithPattern.type_check(&mut type_ctx);

        assert!(matches!(result, Type::Int));
    }
}
