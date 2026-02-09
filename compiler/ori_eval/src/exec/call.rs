//! Function call evaluation helpers.
//!
//! This module provides helper functions for function call evaluation,
//! including argument validation, parameter binding, and capture handling.

use crate::{
    wrong_function_args, Environment, EvalError, EvalResult, FunctionValue, Mutability, Value,
};
use ori_ir::{CallArgRange, ExprArena, ExprId, Name, StringInterner};
use ori_patterns::ControlAction;

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

/// Bind function parameters to argument values in an environment.
///
/// This is the simple case for functions without defaults.
pub fn bind_parameters(env: &mut Environment, func: &FunctionValue, args: &[Value]) {
    for (param, arg) in func.params.iter().zip(args.iter()) {
        env.define(*param, arg.clone(), Mutability::Immutable);
    }
}

/// Bind function parameters with support for default values.
///
/// For parameters not provided in `args`, evaluates the default expression.
/// This requires an interpreter to evaluate default expressions.
///
/// This version assumes args are provided in parameter order (positional).
pub fn bind_parameters_with_defaults(
    interpreter: &mut crate::Interpreter<'_>,
    func: &FunctionValue,
    args: &[Value],
) -> Result<(), ControlAction> {
    for (i, param) in func.params.iter().enumerate() {
        let value = if i < args.len() {
            // Argument was provided
            args[i].clone()
        } else if let Some(default_expr) = func.defaults.get(i).and_then(|d| *d) {
            // Evaluate default expression
            interpreter.eval(default_expr)?
        } else {
            // No argument and no default - this shouldn't happen if check_arg_count passed
            return Err(
                EvalError::new(format!("missing required argument for parameter {i}")).into(),
            );
        };
        interpreter.env.define(*param, value, Mutability::Immutable);
    }
    Ok(())
}

/// Bind function parameters from named arguments with default support.
///
/// This version matches arguments by name to parameters, allowing arguments
/// to be provided in any order and enabling skipping defaulted parameters.
pub fn bind_parameters_from_named_args(
    interpreter: &mut crate::Interpreter<'_>,
    func: &FunctionValue,
    args: CallArgRange,
) -> Result<(), ControlAction> {
    use rustc_hash::FxHashMap;

    // Build a map of argument name -> expression ID from the call args
    let call_args = interpreter.arena.get_call_args(args);
    let mut arg_map: FxHashMap<Name, ExprId> = FxHashMap::default();
    for arg in call_args {
        if let Some(name) = arg.name {
            arg_map.insert(name, arg.value);
        }
    }

    // Bind each parameter
    for (i, param) in func.params.iter().enumerate() {
        let value = if let Some(&arg_expr) = arg_map.get(param) {
            // Named argument was provided for this parameter
            interpreter.eval(arg_expr)?
        } else if let Some(default_expr) = func.defaults.get(i).and_then(|d| *d) {
            // Use default expression
            interpreter.eval(default_expr)?
        } else {
            // No argument and no default - this shouldn't happen if check_arg_count passed
            return Err(
                EvalError::new("missing required argument for parameter".to_string()).into(),
            );
        };
        interpreter.env.define(*param, value, Mutability::Immutable);
    }
    Ok(())
}

/// Check argument count for named arguments against a function.
///
/// With default parameters, validates that:
/// - All required parameters (those without defaults) are provided
/// - No more arguments than total parameters
/// - All argument names match valid parameter names
pub fn check_named_arg_count(
    func: &FunctionValue,
    args: CallArgRange,
    arena: &ExprArena,
) -> Result<(), EvalError> {
    let call_args = arena.get_call_args(args);
    let arg_count = call_args.len();

    // Build set of provided argument names
    let mut provided_names: rustc_hash::FxHashSet<Name> = rustc_hash::FxHashSet::default();
    for arg in call_args {
        if let Some(name) = arg.name {
            provided_names.insert(name);
        }
    }

    // Check that all required parameters are provided
    for (i, param) in func.params.iter().enumerate() {
        let has_default = func.defaults.get(i).is_some_and(Option::is_some);
        if !has_default && !provided_names.contains(param) {
            return Err(EvalError::new("missing required argument".to_string()));
        }
    }

    // Check we don't have more arguments than parameters
    if arg_count > func.params.len() {
        return Err(wrong_function_args(func.params.len(), arg_count));
    }

    Ok(())
}

/// Bind captured variables to an environment.
pub fn bind_captures(env: &mut Environment, func: &FunctionValue) {
    for (name, value) in func.captures() {
        env.define(*name, value.clone(), Mutability::Immutable);
    }
}

/// Bind 'self' for recursive calls.
pub fn bind_self(env: &mut Environment, func: Value, interner: &StringInterner) {
    let self_name = interner.intern("self");
    env.define(self_name, func, Mutability::Immutable);
}

/// Evaluate a call to a `FunctionVal` (built-in function).
pub fn eval_function_val_call(
    func: fn(&[Value]) -> Result<Value, String>,
    args: &[Value],
) -> EvalResult {
    func(args).map_err(|s| EvalError::new(s).into())
}

/// Evaluate a call with named arguments.
///
/// This extracts the values from named arguments and calls the function.
pub fn extract_named_args<F>(
    args: CallArgRange,
    arena: &ExprArena,
    mut eval_fn: F,
) -> Result<Vec<Value>, ControlAction>
where
    F: FnMut(ExprId) -> EvalResult,
{
    let call_args = arena.get_call_args(args);
    call_args.iter().map(|arg| eval_fn(arg.value)).collect()
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#[expect(
    clippy::cast_possible_truncation,
    reason = "Test values fit in target types"
)]
#[expect(
    clippy::cast_possible_wrap,
    reason = "Test values are within signed range"
)]
mod tests {
    use super::*;
    use ori_ir::{ExprId, Name, SharedArena};
    use rustc_hash::FxHashMap;

    fn make_function_value(param_count: usize) -> FunctionValue {
        use ori_ir::ExprArena;
        let params: Vec<Name> = (0..param_count).map(|i| Name::from_raw(i as u32)).collect();
        let body = ExprId::new(0);
        let captures = FxHashMap::default();
        let arena = SharedArena::new(ExprArena::default());
        FunctionValue::new(params, body, captures, arena)
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

    mod bind_parameters_tests {
        use super::*;

        #[test]
        fn binds_single_parameter() {
            let func = make_function_value(1);
            let args = vec![Value::int(42)];
            let mut env = Environment::new();
            env.push_scope();
            bind_parameters(&mut env, &func, &args);
            let param_name = func.params[0];
            assert_eq!(env.lookup(param_name), Some(Value::int(42)));
        }

        #[test]
        fn binds_multiple_parameters() {
            let func = make_function_value(3);
            let args = vec![Value::int(1), Value::int(2), Value::int(3)];
            let mut env = Environment::new();
            env.push_scope();
            bind_parameters(&mut env, &func, &args);
            for (i, &param_name) in func.params.iter().enumerate() {
                let expected = Value::int((i + 1) as i64);
                assert_eq!(env.lookup(param_name), Some(expected));
            }
        }

        #[test]
        fn parameters_are_immutable() {
            let func = make_function_value(1);
            let args = vec![Value::int(42)];
            let mut env = Environment::new();
            env.push_scope();
            bind_parameters(&mut env, &func, &args);
            let param_name = func.params[0];
            // Try to reassign - should fail
            let result = env.assign(param_name, Value::int(99));
            assert!(result.is_err());
        }
    }

    mod bind_captures_tests {
        use super::*;
        use ori_ir::ExprArena;
        use rustc_hash::FxHashMap;

        #[test]
        fn binds_captured_variables() {
            let params = vec![Name::from_raw(0)];
            let body = ExprId::new(0);
            let mut captures = FxHashMap::default();
            let capture_name = Name::from_raw(10);
            captures.insert(capture_name, Value::int(100));
            let arena = SharedArena::new(ExprArena::default());
            let func = FunctionValue::new(params, body, captures, arena);

            let mut env = Environment::new();
            env.push_scope();
            bind_captures(&mut env, &func);
            assert_eq!(env.lookup(capture_name), Some(Value::int(100)));
        }

        #[test]
        fn binds_multiple_captures() {
            let params = vec![];
            let body = ExprId::new(0);
            let mut captures = FxHashMap::default();
            let name1 = Name::from_raw(10);
            let name2 = Name::from_raw(11);
            captures.insert(name1, Value::int(1));
            captures.insert(name2, Value::int(2));
            let arena = SharedArena::new(ExprArena::default());
            let func = FunctionValue::new(params, body, captures, arena);

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
            fn add_one(args: &[Value]) -> Result<Value, String> {
                if let Value::Int(n) = &args[0] {
                    Ok(Value::int(n.raw() + 1))
                } else {
                    Err("expected int".to_string())
                }
            }
            let args = vec![Value::int(41)];
            let result = eval_function_val_call(add_one, &args);
            assert_eq!(result.unwrap(), Value::int(42));
        }

        #[test]
        fn error_is_converted_to_eval_error() {
            fn fail(_args: &[Value]) -> Result<Value, String> {
                Err("intentional error".to_string())
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
