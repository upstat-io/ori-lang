//! Validate pattern implementation.
//!
//! `validate(.value: expr, .rules: [...])` - Validate value against rules.

use crate::types::Type;
use crate::eval::{Value, EvalResult, EvalError};
use super::{PatternDefinition, TypeCheckContext, EvalContext, PatternExecutor};

/// The `validate` pattern validates a value against a list of rules.
///
/// Syntax: `validate(.value: x, .rules: [rule1, rule2, ...])`
///
/// Optional: `.on_error: fn` for custom error handling
///
/// Type: `validate(.value: T, .rules: [T -> bool]) -> Result<T, ValidationError>`
pub struct ValidatePattern;

impl PatternDefinition for ValidatePattern {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn required_props(&self) -> &'static [&'static str] {
        &["value", "rules"]
    }

    fn optional_props(&self) -> &'static [&'static str] {
        &["on_error"]
    }

    fn type_check(&self, ctx: &mut TypeCheckContext) -> Type {
        // validate(.value: T, .rules: [T -> bool]) -> Result<T, Error>
        let value_ty = ctx.get_prop_type("value").unwrap_or_else(|| ctx.fresh_var());
        ctx.result_of(value_ty, Type::Error)
    }

    fn evaluate(
        &self,
        ctx: &EvalContext,
        exec: &mut dyn PatternExecutor,
    ) -> EvalResult {
        let on_error = ctx.get_prop_opt("on_error");

        let value = ctx.eval_prop("value", exec)?;
        let rules = ctx.eval_prop("rules", exec)?;

        match rules {
            Value::List(rule_list) => {
                for rule in rule_list.iter() {
                    let result = exec.call(rule.clone(), vec![value.clone()])?;
                    if !result.is_truthy() {
                        if let Some(on_error_expr) = on_error {
                            let error_fn = exec.eval(on_error_expr)?;
                            let error_val = exec.call(error_fn, vec![value.clone()])?;
                            return Ok(Value::err(error_val));
                        } else {
                            return Ok(Value::err(Value::string("validation failed")));
                        }
                    }
                }
                Ok(Value::ok(value))
            }
            _ => Err(EvalError::new("validate .rules must be a list of predicates")),
        }
    }

}

impl ValidatePattern {
    /// Create a validation error result.
    pub fn validation_error(message: &str) -> Value {
        Value::err(Value::string(message))
    }
}
