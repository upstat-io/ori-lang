//! Function and method call evaluation.
//!
//! This module handles function calls, method dispatch, and named argument calls.
//! It's designed to be called from the main Evaluator.

use crate::ir::{Name, StringInterner, ExprArena, CallArgRange};
use crate::eval::{Value, EvalResult, EvalError, FunctionValue, Environment};
use crate::eval::errors;
use crate::eval::methods::MethodRegistry;
use crate::context::SharedRegistry;

/// Configuration for function call evaluation.
pub struct CallContext<'a> {
    pub interner: &'a StringInterner,
    pub arena: &'a ExprArena,
    pub method_registry: &'a SharedRegistry<MethodRegistry>,
}

/// Evaluate a method call.
///
/// Delegates to the MethodRegistry for dispatch.
pub fn eval_method_call(
    receiver: Value,
    method: Name,
    args: Vec<Value>,
    ctx: &CallContext<'_>,
) -> EvalResult {
    let method_name = ctx.interner.lookup(method);
    ctx.method_registry.dispatch(receiver, method_name, args)
}

/// Check if a function has the correct argument count.
pub fn check_arg_count(func: &FunctionValue, args: &[Value]) -> Result<(), EvalError> {
    if args.len() != func.params.len() {
        return Err(errors::wrong_function_args(func.params.len(), args.len()));
    }
    Ok(())
}

/// Bind function parameters to argument values in an environment.
pub fn bind_parameters(env: &mut Environment, func: &FunctionValue, args: &[Value]) {
    for (param, arg) in func.params.iter().zip(args.iter()) {
        env.define(*param, arg.clone(), false);
    }
}

/// Bind captured variables to an environment.
pub fn bind_captures(env: &mut Environment, func: &FunctionValue) {
    for (name, value) in func.captures() {
        env.define(*name, value.clone(), false);
    }
}

/// Bind 'self' for recursive calls.
pub fn bind_self(env: &mut Environment, func: Value, interner: &StringInterner) {
    let self_name = interner.intern("self");
    env.define(self_name, func, false);
}

/// Evaluate a call to a FunctionVal (built-in function).
pub fn eval_function_val_call(
    func: fn(&[Value]) -> Result<Value, String>,
    args: &[Value],
) -> EvalResult {
    func(args).map_err(EvalError::new)
}

/// Evaluate a call with named arguments.
///
/// This extracts the values from named arguments and calls the function.
pub fn extract_named_args<F>(
    args: CallArgRange,
    arena: &ExprArena,
    mut eval_fn: F,
) -> Result<Vec<Value>, EvalError>
where
    F: FnMut(crate::ir::ExprId) -> EvalResult,
{
    let call_args = arena.get_call_args(args);
    call_args.iter()
        .map(|arg| eval_fn(arg.value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::SharedInterner;

    #[test]
    fn test_check_arg_count_correct() {
        let interner = SharedInterner::default();
        let name = interner.intern("x");
        let func = FunctionValue::new(vec![name], crate::ir::ExprId::new(0));
        let args = vec![Value::Int(1)];
        assert!(check_arg_count(&func, &args).is_ok());
    }

    #[test]
    fn test_check_arg_count_wrong() {
        let interner = SharedInterner::default();
        let name = interner.intern("x");
        let func = FunctionValue::new(vec![name], crate::ir::ExprId::new(0));
        let args = vec![Value::Int(1), Value::Int(2)];
        assert!(check_arg_count(&func, &args).is_err());
    }

    #[test]
    fn test_bind_parameters() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");
        let func = FunctionValue::new(vec![x, y], crate::ir::ExprId::new(0));
        let args = vec![Value::Int(1), Value::Int(2)];

        let mut env = Environment::new();
        env.push_scope();
        bind_parameters(&mut env, &func, &args);

        assert_eq!(env.lookup(x), Some(Value::Int(1)));
        assert_eq!(env.lookup(y), Some(Value::Int(2)));
    }

    #[test]
    fn test_bind_self() {
        let interner = SharedInterner::default();
        let name = interner.intern("test");
        let func = FunctionValue::new(vec![name], crate::ir::ExprId::new(0));
        let func_val = Value::Function(func);

        let mut env = Environment::new();
        env.push_scope();
        bind_self(&mut env, func_val.clone(), &interner);

        let self_name = interner.intern("self");
        assert!(env.lookup(self_name).is_some());
    }

    #[test]
    fn test_eval_function_val_call_success() {
        fn add_one(args: &[Value]) -> Result<Value, String> {
            if let Value::Int(n) = &args[0] {
                Ok(Value::Int(n + 1))
            } else {
                Err("expected int".to_string())
            }
        }

        let result = eval_function_val_call(add_one, &[Value::Int(5)]);
        assert_eq!(result.unwrap(), Value::Int(6));
    }

    #[test]
    fn test_eval_function_val_call_error() {
        fn always_error(_args: &[Value]) -> Result<Value, String> {
            Err("always fails".to_string())
        }

        let result = eval_function_val_call(always_error, &[]);
        assert!(result.is_err());
    }
}
