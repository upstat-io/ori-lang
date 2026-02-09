//! Unary operator implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for unary operations. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use ori_ir::UnaryOp;
use ori_patterns::{integer_overflow, ControlAction, EvalError, EvalResult, Value};

/// Evaluate a unary operation using direct pattern matching.
///
/// This is the preferred entry point for unary operations. It uses
/// enum-based dispatch which is faster than trait objects for fixed type sets.
pub fn evaluate_unary(value: Value, op: UnaryOp) -> EvalResult {
    match (&value, op) {
        // Numeric negation
        (Value::Int(n), UnaryOp::Neg) => n
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("negation").into()),
        (Value::Float(f), UnaryOp::Neg) => Ok(Value::Float(-f)),
        (Value::Duration(d), UnaryOp::Neg) => d
            .checked_neg()
            .map(Value::Duration)
            .ok_or_else(|| integer_overflow("duration negation").into()),

        // Logical not
        (Value::Bool(b), UnaryOp::Not) => Ok(Value::Bool(!b)),

        // Bitwise not
        (Value::Int(n), UnaryOp::BitNot) => Ok(Value::Int(!*n)),

        // Try operator (?) for Option and Result types
        (_, UnaryOp::Try) => eval_try(value),

        // Invalid combinations
        _ => Err(invalid_unary_op(value.type_name(), op).into()),
    }
}

/// Try operator (?) for Option and Result types.
///
/// For Option types: unwraps Some, propagates None
/// For Result types: unwraps Ok, propagates Err
/// For other types: passes through unchanged (for compatibility)
fn eval_try(value: Value) -> EvalResult {
    match value {
        Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
        Value::Err(e) => Err(ControlAction::Propagate(Value::Err(e))),
        Value::None => Err(ControlAction::Propagate(Value::None)),
        other => Ok(other),
    }
}

/// Create an error for invalid unary operations.
#[cold]
fn invalid_unary_op(type_name: &str, op: UnaryOp) -> EvalError {
    EvalError::new(format!("invalid unary {op:?} on {type_name}"))
}

// Tests relocated to tests/unary_operators_tests.rs per coding guidelines (>200 lines)
