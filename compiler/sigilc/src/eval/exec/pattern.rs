//! Pattern evaluation (function_seq and function_exp).
//!
//! This module handles Sigil's pattern constructs:
//!
//! **function_seq** (sequential expressions):
//! - `run(...)` - sequential evaluation
//! - `try(...)` - error handling with early return
//! - `match(...)` - pattern matching (delegated to control.rs)
//!
//! **function_exp** (named expressions):
//! - `map`, `filter`, `fold`, `find`, `collect`
//! - `parallel`, `spawn`, `timeout`, `retry`
//! - `recurse`, `cache`, `validate`, `with`
//!
//! These are evaluated via the PatternRegistry which implements
//! the Open/Closed principle for extensibility.

use crate::ir::{ExprId, BindingPattern, FunctionSeq, SeqBinding, SeqBindingRange, ExprArena};
use crate::eval::{Value, EvalResult, EvalError};
use crate::eval::environment::Environment;

/// Evaluate a run pattern (sequential evaluation).
///
/// Evaluates bindings in sequence, then returns the result.
pub fn eval_run<F, G>(
    bindings: SeqBindingRange,
    result: ExprId,
    arena: &ExprArena,
    env: &mut Environment,
    mut eval_fn: F,
    mut bind_fn: G,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
    G: FnMut(&BindingPattern, Value, bool, &mut Environment) -> EvalResult,
{
    let seq_bindings = arena.get_seq_bindings(bindings);
    for binding in seq_bindings {
        match binding {
            SeqBinding::Let { pattern, value, mutable, .. } => {
                let val = eval_fn(*value)?;
                bind_fn(pattern, val, *mutable, env)?;
            }
            SeqBinding::Stmt { expr, .. } => {
                // Evaluate for side effects (e.g., assignment)
                eval_fn(*expr)?;
            }
        }
    }
    // Evaluate and return result
    eval_fn(result)
}

/// Evaluate a try pattern (error handling with early return).
///
/// Evaluates bindings, unwrapping Result/Option types.
/// Returns early on Err or None values.
pub fn eval_try<F, G>(
    bindings: SeqBindingRange,
    result: ExprId,
    arena: &ExprArena,
    env: &mut Environment,
    mut eval_fn: F,
    mut bind_fn: G,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult,
    G: FnMut(&BindingPattern, Value, bool, &mut Environment) -> EvalResult,
{
    let seq_bindings = arena.get_seq_bindings(bindings);
    for binding in seq_bindings {
        match binding {
            SeqBinding::Let { pattern, value, mutable, .. } => {
                match eval_fn(*value) {
                    Ok(value) => {
                        // Unwrap Result/Option types per spec:
                        // "If any binding expression returns a Result<T, E>, the binding variable has type T"
                        let unwrapped = match value {
                            Value::Ok(inner) => (*inner).clone(),
                            Value::Err(e) => {
                                // Early return with the error
                                return Ok(Value::Err(e));
                            }
                            Value::Some(inner) => (*inner).clone(),
                            Value::None => {
                                // Early return with None
                                return Ok(Value::None);
                            }
                            other => other,
                        };
                        bind_fn(pattern, unwrapped, *mutable, env)?;
                    }
                    Err(e) => {
                        // If this is a propagated error, return the value
                        if let Some(propagated) = e.propagated_value {
                            return Ok(propagated);
                        }
                        return Err(e);
                    }
                }
            }
            SeqBinding::Stmt { expr, .. } => {
                // Evaluate for side effects
                eval_fn(*expr)?;
            }
        }
    }
    // Evaluate and return result
    eval_fn(result)
}

/// Evaluate a function_seq expression.
///
/// Dispatches to the appropriate pattern evaluator based on the variant.
pub fn eval_function_seq<F, G, M>(
    func_seq: &FunctionSeq,
    arena: &ExprArena,
    env: &mut Environment,
    eval_fn: F,
    bind_fn: G,
    match_fn: M,
) -> EvalResult
where
    F: FnMut(ExprId) -> EvalResult + Clone,
    G: FnMut(&BindingPattern, Value, bool, &mut Environment) -> EvalResult + Clone,
    M: FnOnce(Value, crate::ir::ArmRange) -> EvalResult,
{
    match func_seq {
        FunctionSeq::Run { bindings, result, .. } => {
            eval_run(*bindings, *result, arena, env, eval_fn, bind_fn)
        }
        FunctionSeq::Try { bindings, result, .. } => {
            eval_try(*bindings, *result, arena, env, eval_fn, bind_fn)
        }
        FunctionSeq::Match { scrutinee, arms, .. } => {
            let mut eval = eval_fn.clone();
            let value = eval(*scrutinee)?;
            match_fn(value, *arms)
        }
        FunctionSeq::ForPattern { default, .. } => {
            // TODO: Implement for pattern evaluation
            // For now, evaluate and return the default value
            let mut eval = eval_fn.clone();
            eval(*default)
        }
    }
}

/// Evaluate a try expression (? operator).
///
/// Unwraps Ok/Some values, propagates Err/None.
pub fn eval_try_expr(value: Value, value_to_string: impl Fn(&Value) -> String) -> EvalResult {
    match value {
        Value::Ok(v) => Ok((*v).clone()),
        Value::Err(e) => Err(EvalError::propagate(
            Value::Err(e.clone()),
            format!("propagated error: {}", value_to_string(&e))
        )),
        Value::Some(v) => Ok((*v).clone()),
        Value::None => Err(EvalError::propagate(Value::None, "propagated None")),
        other => Ok(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_try_expr_ok() {
        let value = Value::ok(Value::Int(42));
        let result = eval_try_expr(value, |_| "".to_string());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_try_expr_err() {
        let value = Value::err(Value::string("error"));
        let result = eval_try_expr(value, |_| "error".to_string());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.propagated_value.is_some());
    }

    #[test]
    fn test_eval_try_expr_some() {
        let value = Value::some(Value::Int(42));
        let result = eval_try_expr(value, |_| "".to_string());
        assert_eq!(result.unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_try_expr_none() {
        let value = Value::None;
        let result = eval_try_expr(value, |_| "".to_string());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.propagated_value.is_some());
        assert!(matches!(err.propagated_value.unwrap(), Value::None));
    }

    #[test]
    fn test_eval_try_expr_passthrough() {
        let value = Value::Int(42);
        let result = eval_try_expr(value, |_| "".to_string());
        assert_eq!(result.unwrap(), Value::Int(42));
    }
}
