//! Print pattern implementation.
//!
//! The `print` pattern uses the `Print` capability to output text.
//! This allows output to be redirected (e.g., to a buffer in WASM or tests).

use ori_types::Type;

use crate::{
    EvalContext, EvalResult, PatternDefinition, PatternExecutor, TypeCheckContext, Value,
};

/// The `print` pattern prints a message using the Print capability.
///
/// Syntax: `print(msg: expr)`
/// Type: `print(msg: str) -> void`
///
/// The Print capability determines where output goes:
/// - Native: stdout (default)
/// - WASM: buffer for capture
/// - Tests: buffer for assertions
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
        let msg_str = msg.display_value();

        // Look up Print capability and call its println method
        if let Some(print_cap) = exec.lookup_capability("Print") {
            exec.call_method(print_cap, "println", vec![Value::string(msg_str)])?;
        } else {
            // Fallback: no Print capability, use default output
            // This calls a built-in that the interpreter provides
            exec.call_method(Value::Void, "__builtin_println", vec![Value::string(msg_str)])?;
        }

        Ok(Value::Void)
    }
}
