//! Pattern evaluation (`function_seq` and `function_exp`).
//!
//! This module handles Ori's pattern constructs:
//!
//! **`function_seq`** (sequential expressions):
//! - `run(...)` - sequential evaluation
//! - `try(...)` - error handling with early return
//! - `match(...)` - pattern matching (delegated to control.rs)
//!
//! **`function_exp`** (named expressions):
//! - `map`, `filter`, `fold`, `find`, `collect`
//! - `parallel`, `spawn`, `timeout`, `retry`
//! - `recurse`, `cache`, `validate`, `with`
//!
//! These are evaluated via the `PatternRegistry` which implements
//! the Open/Closed principle for extensibility.
//!
//! Note: The `eval_run`, `eval_try`, and `eval_function_seq` functions that
//! previously lived here have been superseded by `interpreter/function_seq.rs`.

#[cfg(test)]
use ori_patterns::ControlAction;

#[cfg(test)]
use crate::{EvalResult, Value};

/// Evaluate a try expression (? operator).
///
/// Unwraps Ok/Some values, propagates Err/None.
#[cfg(test)]
fn eval_try_expr(value: Value) -> EvalResult {
    match value {
        Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
        Value::Err(e) => Err(ControlAction::Propagate(Value::Err(e))),
        Value::None => Err(ControlAction::Propagate(Value::None)),
        other => Ok(other),
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_eval_try_expr_ok() {
        let value = Value::ok(Value::int(42));
        let result = eval_try_expr(value);
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn test_eval_try_expr_err() {
        let value = Value::err(Value::string("error"));
        let result = eval_try_expr(value);
        assert!(result.is_err());
        let action = result.unwrap_err();
        assert!(matches!(action, ControlAction::Propagate(Value::Err(_))));
    }

    #[test]
    fn test_eval_try_expr_some() {
        let value = Value::some(Value::int(42));
        let result = eval_try_expr(value);
        assert_eq!(result.unwrap(), Value::int(42));
    }

    #[test]
    fn test_eval_try_expr_none() {
        let value = Value::None;
        let result = eval_try_expr(value);
        assert!(result.is_err());
        let action = result.unwrap_err();
        assert!(matches!(action, ControlAction::Propagate(Value::None)));
    }

    #[test]
    fn test_eval_try_expr_passthrough() {
        let value = Value::int(42);
        let result = eval_try_expr(value);
        assert_eq!(result.unwrap(), Value::int(42));
    }
}
