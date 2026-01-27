//! Catch pattern implementation.

use ori_types::Type;

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext, Value};

/// The `catch` pattern captures panics and converts them to `Result<T, str>`.
///
/// Syntax: `catch(expr: expression)`
/// Type: `catch(expr: T) -> Result<T, str>`
pub struct CatchPattern;

impl PatternDefinition for CatchPattern {
    fn name(&self) -> &'static str {
        "catch"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["expr"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        // TODO: Infer T from the expr property type.
        // For now, return Result<int, str> as a placeholder.
        Type::Result {
            ok: Box::new(Type::Int),
            err: Box::new(Type::Str),
        }
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        match ctx.eval_prop("expr", exec) {
            Ok(value) => Ok(Value::ok(value)),
            Err(e) => Ok(Value::err(Value::string(e.message))),
        }
    }
}
