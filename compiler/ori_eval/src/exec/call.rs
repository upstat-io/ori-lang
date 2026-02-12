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
mod tests {
    use super::*;
    use ori_ir::{Name, SharedArena};
    use rustc_hash::FxHashMap;

    fn make_function_value(param_count: usize) -> FunctionValue {
        use ori_ir::ExprArena;
        let params: Vec<Name> = (0..param_count).map(|i| Name::from_raw(i as u32)).collect();
        let captures = FxHashMap::default();
        let arena = SharedArena::new(ExprArena::default());
        FunctionValue::new(params, captures, arena)
    }

    mod check_arg_count_tests {
        use super::*;

        #[test]
        fn correct_count_returns_ok() {
            let func = make_function_value(2);
            let args = vec![Value::int(1), Value::int(2)];
            assert!(check_arg_count(&func, &args).is_ok());
        }

        #[test]
        fn too_few_args_returns_error() {
            let func = make_function_value(2);
            let args = vec![Value::int(1)];
            let result = check_arg_count(&func, &args);
            assert!(result.is_err());
        }

        #[test]
        fn too_many_args_returns_error() {
            let func = make_function_value(1);
            let args = vec![Value::int(1), Value::int(2)];
            let result = check_arg_count(&func, &args);
            assert!(result.is_err());
        }

        #[test]
        fn zero_params_zero_args_ok() {
            let func = make_function_value(0);
            let args: Vec<Value> = vec![];
            assert!(check_arg_count(&func, &args).is_ok());
        }
    }

    mod bind_captures_tests {
        use super::*;
        use ori_ir::ExprArena;
        use rustc_hash::FxHashMap;

        #[test]
        fn binds_captured_variables() {
            let params = vec![Name::from_raw(0)];
            let mut captures = FxHashMap::default();
            let capture_name = Name::from_raw(10);
            captures.insert(capture_name, Value::int(100));
            let arena = SharedArena::new(ExprArena::default());
            let func = FunctionValue::new(params, captures, arena);

            let mut env = Environment::new();
            env.push_scope();
            bind_captures(&mut env, &func);
            assert_eq!(env.lookup(capture_name), Some(Value::int(100)));
        }

        #[test]
        fn binds_multiple_captures() {
            let params = vec![];
            let mut captures = FxHashMap::default();
            let name1 = Name::from_raw(10);
            let name2 = Name::from_raw(11);
            captures.insert(name1, Value::int(1));
            captures.insert(name2, Value::int(2));
            let arena = SharedArena::new(ExprArena::default());
            let func = FunctionValue::new(params, captures, arena);

            let mut env = Environment::new();
            env.push_scope();
            bind_captures(&mut env, &func);
            assert_eq!(env.lookup(name1), Some(Value::int(1)));
            assert_eq!(env.lookup(name2), Some(Value::int(2)));
        }
    }

    mod eval_function_val_call_tests {
        use super::*;

        #[test]
        fn successful_call_returns_value() {
            fn add_one(args: &[Value]) -> Result<Value, EvalError> {
                if let Value::Int(n) = &args[0] {
                    Ok(Value::int(n.raw() + 1))
                } else {
                    Err(EvalError::new("expected int"))
                }
            }
            let args = vec![Value::int(41)];
            let result = eval_function_val_call(add_one, &args);
            assert_eq!(result.unwrap(), Value::int(42));
        }

        #[test]
        fn error_is_converted_to_eval_error() {
            fn fail(_args: &[Value]) -> Result<Value, EvalError> {
                Err(EvalError::new("intentional error"))
            }
            let args: Vec<Value> = vec![];
            let result = eval_function_val_call(fail, &args);
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err().into_eval_error().message,
                "intentional error"
            );
        }
    }
}
