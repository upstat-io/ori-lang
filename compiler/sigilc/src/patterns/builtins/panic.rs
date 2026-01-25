//! Panic pattern implementation.

use crate::types::Type;
use crate::eval::{EvalResult, EvalError};
use crate::patterns::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

/// The `panic` pattern halts execution with an error message.
///
/// Syntax: `panic(msg: expr)`
/// Type: `panic(msg: str) -> Never`
pub struct PanicPattern;

impl PatternDefinition for PanicPattern {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["msg"]
    }

    fn type_check(&self, _ctx: &mut TypeCheckContext) -> Type {
        Type::Never
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = ctx.eval_prop("msg", exec)?;
        Err(EvalError::new(format!("panic: {}", msg.display_value())))
    }
}
