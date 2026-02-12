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
// Tests use unwrap() to panic on unexpected state, making failures immediately visible
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::test_helpers::{make_ctx, MockPatternExecutor};
    use ori_ir::{ExprArena, ExprId, NamedExpr, SharedInterner, Span};

    #[test]
    fn print_returns_void() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let msg_name = interner.intern("msg");
        let props = vec![NamedExpr {
            name: msg_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        }];

        let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::string("hello"));

        let ctx = make_ctx(&interner, &arena, &props);
        let result = PrintPattern.evaluate(&ctx, &mut exec).unwrap();

        assert!(matches!(result, Value::Void));
    }

    #[test]
    fn print_uses_capability_when_available() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let msg_name = interner.intern("msg");
        let props = vec![NamedExpr {
            name: msg_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        }];

        // Mock with Print capability
        let print_cap_name = interner.intern("Print");
        let mut exec = MockPatternExecutor::new()
            .with_expr(ExprId::new(0), Value::string("test"))
            .with_capability(print_cap_name, Value::Void);

        let ctx = make_ctx(&interner, &arena, &props);
        let result = PrintPattern.evaluate(&ctx, &mut exec);

        // Should succeed (call_method returns Ok(Void) in mock)
        assert!(result.is_ok());
    }

    #[test]
    fn print_falls_back_when_no_capability() {
        let interner = SharedInterner::default();
        let arena = ExprArena::new();
        let msg_name = interner.intern("msg");
        let props = vec![NamedExpr {
            name: msg_name,
            value: ExprId::new(0),
            span: Span::new(0, 0),
        }];

        // No Print capability registered
        let mut exec = MockPatternExecutor::new().with_expr(ExprId::new(0), Value::string("test"));

        let ctx = make_ctx(&interner, &arena, &props);
        let result = PrintPattern.evaluate(&ctx, &mut exec);

        // Should succeed using __builtin_println fallback
        assert!(result.is_ok());
    }

    #[test]
    fn print_pattern_name() {
        assert_eq!(PrintPattern.name(), "print");
    }

    #[test]
    fn print_required_props() {
        assert_eq!(PrintPattern.required_props(), &["msg"]);
    }
}
