//! Print pattern implementation.

use sigil_types::Type;

use crate::{
    EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext, Value,
};

/// The `print` pattern prints a message to stdout.
///
/// Syntax: `print(msg: expr)`
/// Type: `print(msg: str) -> void`
pub struct PrintPattern;

impl PatternDefinition for PrintPattern {
    fn name(&self) -> &'static str {
        "print"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["msg"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Unit
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = ctx.eval_prop("msg", exec)?;
        println!("{}", msg.display_value());
        Ok(Value::Void)
    }
}
