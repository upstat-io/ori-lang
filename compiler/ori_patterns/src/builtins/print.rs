//! Print pattern implementation.
//!
//! The `print` pattern uses the `Print` capability to output text.
//! This allows output to be redirected (e.g., to a buffer in WASM or tests).

use crate::{EvalContext, EvalResult, PatternDefinition, PatternExecutor, Value};

/// The `print` pattern prints a message using the Print capability.
///
/// Syntax: `print(msg: expr)`
/// Type: `print(msg: str) -> void`
///
/// The Print capability determines where output goes:
/// - Native: stdout (default)
/// - WASM: buffer for capture
/// - Tests: buffer for assertions
#[derive(Clone, Copy)]
pub struct PrintPattern;

impl PatternDefinition for PrintPattern {
    fn name(&self) -> &'static str {
        "print"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["msg"]
    }

    fn evaluate(&self, ctx: &EvalContext, exec: &mut dyn PatternExecutor) -> EvalResult {
        let msg = ctx.eval_prop("msg", exec)?;
        let msg_str = msg.display_value();

        let print_name = ctx.interner.intern("Print");
        let println_name = ctx.interner.intern("println");

        // Look up Print capability and call its println method
        if let Some(print_cap) = exec.lookup_capability(print_name) {
            exec.call_method(print_cap, println_name, vec![Value::string(msg_str)])?;
        } else {
            // Fallback: no Print capability, use default output
            // This calls a built-in that the interpreter provides
            let builtin_name = ctx.interner.intern("__builtin_println");
            exec.call_method(Value::Void, builtin_name, vec![Value::string(msg_str)])?;
        }

        Ok(Value::Void)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "tests use unwrap to panic on unexpected state"
)]
mod tests;
