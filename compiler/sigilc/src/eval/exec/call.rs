//! Function and method call evaluation.
//!
//! This module handles function calls, method dispatch, and named argument calls.
//! It's designed to be called from the main Evaluator.

use crate::ir::{Name, StringInterner, ExprArena, CallArgRange};
use crate::eval::errors::wrong_function_args;
use crate::eval::{Value, FunctionValue, Environment, EvalResult, EvalError};
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
/// Delegates to the `MethodRegistry` for dispatch.
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
    F: FnMut(crate::ir::ExprId) -> EvalResult,
{
    let call_args = arena.get_call_args(args);
    call_args.iter()
        .map(|arg| eval_fn(arg.value))
        .collect()
}
