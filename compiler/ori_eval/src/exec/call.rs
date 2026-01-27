//! Function call evaluation helpers.
//!
//! This module provides helper functions for function call evaluation,
//! including argument validation, parameter binding, and capture handling.

use crate::{wrong_function_args, Environment, EvalError, EvalResult, FunctionValue, Value};
use ori_ir::{CallArgRange, ExprArena, ExprId, StringInterner};

/// Check if a function has the correct argument count.
pub fn check_arg_count(func: &FunctionValue, args: &[Value]) -> Result<(), EvalError> {
    if args.len() != func.params.len() {
        return Err(wrong_function_args(func.params.len(), args.len()));
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

/// Evaluate a call to a `FunctionVal` (built-in function).
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
    F: FnMut(ExprId) -> EvalResult,
{
    let call_args = arena.get_call_args(args);
    call_args.iter().map(|arg| eval_fn(arg.value)).collect()
}
