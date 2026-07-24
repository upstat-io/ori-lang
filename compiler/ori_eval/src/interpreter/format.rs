//! Format specification evaluation for template string interpolation.
//!
//! Handles `CanExpr::FormatWith { expr, spec }` by evaluating the expression,
//! parsing the format spec, and applying type-specific formatting via the
//! shared `ori_format` emission kernel (the single source of truth both
//! execution backends call).
//!
//! Only primitive values reach here — `ori_canon` desugars every non-primitive
//! `{expr:spec}` interpolation to a user `format()` `MethodCall` or a `to_str()`
//! + str-FormatWith re-route.

use ori_format::{
    format_bool, format_char, format_float, format_int, format_str, parse_format_spec,
};
use ori_ir::canon::CanId;
use ori_ir::Name;
use ori_patterns::{EvalError, EvalResult, Value};

use super::Interpreter;

impl Interpreter<'_> {
    /// Evaluate a `FormatWith` expression: format `expr` using `spec`.
    ///
    /// Only the five primitive types (int, float, str, bool, char) reach here.
    /// `ori_canon` desugars every non-primitive `{expr:spec}` to a user
    /// `format()` `MethodCall` (explicit Formattable) or a `to_str()` + str
    /// `FormatWith` re-route (blanket Printable), so a non-primitive value here
    /// is an `ori_canon` bug.
    pub(super) fn eval_format_with(
        &mut self,
        can_id: CanId,
        expr: CanId,
        spec: Name,
    ) -> EvalResult {
        let value = self.eval_can(expr)?;
        let spec_str = self.interner.lookup(spec);

        let parsed = parse_format_spec(spec_str).map_err(|e| {
            let span = self.can_span(can_id);
            Self::attach_span(
                EvalError::new(format!("invalid format spec: {e}")).into(),
                span,
            )
        })?;

        // Only primitive FormatWith reaches the interpreter: ori_canon desugars
        // every non-primitive `{expr:spec}` to either a user `format()`
        // MethodCall or a `to_str()` + str-FormatWith re-route. The catch-all
        // unreachable keeps a missed canon path loud rather than silently
        // falling back to `display_value()`.
        let result = match &value {
            Value::Int(n) => format_int(n.raw(), &parsed),
            Value::Float(f) => format_float(*f, &parsed),
            Value::Str(s) => format_str(s, &parsed),
            Value::Bool(b) => format_bool(*b, &parsed),
            Value::Char(c) => format_char(*c, &parsed),
            other => {
                unreachable!(
                    "non-primitive FormatWith reached interpreter (desugared in ori_canon): {other:?}"
                )
            }
        };

        Ok(Value::string(result))
    }
}
