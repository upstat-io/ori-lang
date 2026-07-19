//! Option/Result closure-taking method implementations.
//!
//! These methods (`map`, `and_then`, `filter`, `or_else`, `map_err`) need
//! `&mut self` because they call user-provided closures via `eval_call`.

use crate::errors::wrong_arg_count;
use crate::{EvalResult, Value};

use super::super::super::Interpreter;

impl Interpreter<'_> {
    // Option closure methods

    /// `Option<T>.map(f: T -> U) -> Option<U>`
    pub(super) fn eval_option_map(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("map", 1, args.len()).into());
        }
        match receiver {
            Value::Some(v) => {
                let result = self.eval_call(&args[0], &[(*v).clone()])?;
                Ok(Value::some(result))
            }
            Value::None => Ok(Value::None),
            _ => unreachable!("OptionMap dispatched on non-Option"),
        }
    }

    /// `Option<T>.and_then(f: T -> Option<U>) -> Option<U>`
    pub(super) fn eval_option_and_then(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("and_then", 1, args.len()).into());
        }
        match receiver {
            Value::Some(v) => self.eval_call(&args[0], &[(*v).clone()]),
            Value::None => Ok(Value::None),
            _ => unreachable!("OptionAndThen dispatched on non-Option"),
        }
    }

    /// `Option<T>.filter(predicate: T -> bool) -> Option<T>`
    pub(super) fn eval_option_filter(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("filter", 1, args.len()).into());
        }
        match &receiver {
            Value::Some(v) => {
                let keep = self.eval_call(&args[0], std::slice::from_ref(v.as_ref()))?;
                if keep.is_truthy() {
                    Ok(receiver)
                } else {
                    Ok(Value::None)
                }
            }
            Value::None => Ok(Value::None),
            _ => unreachable!("OptionFilter dispatched on non-Option"),
        }
    }

    /// `Option<T>.or_else(f: () -> Option<T>) -> Option<T>`
    pub(super) fn eval_option_or_else(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("or_else", 1, args.len()).into());
        }
        match &receiver {
            Value::Some(_) => Ok(receiver),
            Value::None => self.eval_call(&args[0], &[]),
            _ => unreachable!("OptionOrElse dispatched on non-Option"),
        }
    }

    // Result closure methods

    /// `Result<T, E>.map(f: T -> U) -> Result<U, E>`
    pub(super) fn eval_result_map(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("map", 1, args.len()).into());
        }
        match receiver {
            Value::Ok(v) => {
                let result = self.eval_call(&args[0], &[(*v).clone()])?;
                Ok(Value::ok(result))
            }
            Value::Err(_) => Ok(receiver),
            _ => unreachable!("ResultMap dispatched on non-Result"),
        }
    }

    /// `Result<T, E>.map_err(f: E -> F) -> Result<T, F>`
    pub(super) fn eval_result_map_err(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("map_err", 1, args.len()).into());
        }
        match receiver {
            Value::Err(e) => {
                let result = self.eval_call(&args[0], &[(*e).clone()])?;
                Ok(Value::err(result))
            }
            Value::Ok(_) => Ok(receiver),
            _ => unreachable!("ResultMapErr dispatched on non-Result"),
        }
    }

    /// `Result<T, E>.and_then(f: T -> Result<U, E>) -> Result<U, E>`
    pub(super) fn eval_result_and_then(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("and_then", 1, args.len()).into());
        }
        match receiver {
            Value::Ok(v) => self.eval_call(&args[0], &[(*v).clone()]),
            Value::Err(_) => Ok(receiver),
            _ => unreachable!("ResultAndThen dispatched on non-Result"),
        }
    }

    /// `Result<T, E>.or_else(f: E -> Result<T, F>) -> Result<T, F>`
    pub(super) fn eval_result_or_else(&mut self, receiver: Value, args: &[Value]) -> EvalResult {
        if args.len() != 1 {
            return Err(wrong_arg_count("or_else", 1, args.len()).into());
        }
        match receiver {
            Value::Err(e) => self.eval_call(&args[0], &[(*e).clone()]),
            Value::Ok(_) => Ok(receiver),
            _ => unreachable!("ResultOrElse dispatched on non-Result"),
        }
    }
}
