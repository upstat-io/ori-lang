//! Function call evaluation helpers.
//!
//! This module provides helper functions for function call evaluation,
//! including argument validation, parameter binding, and capture handling.

use ori_patterns::ControlAction;

use crate::errors::wrong_function_args;
use crate::{Environment, EvalError, EvalResult, FunctionValue, Mutability, Value};

/// Check if a function has the correct argument count.
///
/// With default parameters, the valid range is:
/// - Minimum: number of parameters without defaults (required parameters)
/// - Maximum: total number of parameters
pub fn check_arg_count(func: &FunctionValue, args: &[Value]) -> Result<(), EvalError> {
    let required = func.required_param_count();
    let total = func.params.len();

    if args.len() < required || args.len() > total {
        if required == total {
            // No defaults - show exact count expected
            return Err(wrong_function_args(total, args.len()));
        }
        // With defaults - show range
        return Err(wrong_function_args_range(required, total, args.len()));
    }
    Ok(())
}

/// Create an error for wrong argument count with a range.
fn wrong_function_args_range(min: usize, max: usize, got: usize) -> EvalError {
    EvalError::new(format!("expected {min} to {max} arguments, got {got}"))
}

/// Bind function parameters with support for default values.
///
/// For parameters not provided in `args`, evaluates the default
/// expression via `eval_can()`. Assumes args are positional.
pub fn bind_parameters_with_defaults(
    interpreter: &mut crate::Interpreter<'_>,
    func: &FunctionValue,
    args: &[Value],
) -> Result<(), ControlAction> {
    let can_defaults = func.can_defaults();
    for (i, param) in func.params.iter().enumerate() {
        let value = if i < args.len() {
            args[i].clone()
        } else if let Some(Some(can_id)) = can_defaults.get(i) {
            interpreter.eval_can(*can_id)?
        } else {
            return Err(
                EvalError::new(format!("missing required argument for parameter {i}")).into(),
            );
        };
        interpreter.env.define(*param, value, Mutability::Immutable);
    }
    Ok(())
}

/// Bind captured variables from a name→value iterator.
///
/// Both `FunctionValue::captures()` and `UserMethod.captures.iter()` produce
/// compatible iterators of `(&Name, &Value)`. This shared helper avoids
/// duplicating the capture binding logic across function and method call paths.
pub fn bind_captures_iter<'a>(
    env: &mut Environment,
    captures: impl Iterator<Item = (&'a ori_ir::Name, &'a Value)>,
) {
    for (name, value) in captures {
        env.define(*name, value.clone(), Mutability::Immutable);
    }
}

/// Bind captured variables from a `FunctionValue` to an environment.
pub fn bind_captures(env: &mut Environment, func: &FunctionValue) {
    bind_captures_iter(env, func.captures());
}

/// Evaluate a call to a `FunctionVal` (built-in function).
pub fn eval_function_val_call(
    func: fn(&[Value]) -> Result<Value, EvalError>,
    args: &[Value],
) -> EvalResult {
    func(args).map_err(ControlAction::from)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#[expect(
    clippy::cast_possible_truncation,
    reason = "Test values fit in target types"
)]
mod tests;
