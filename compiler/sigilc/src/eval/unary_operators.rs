//! Unary operator implementations for the evaluator.
//!
//! This module extracts unary operation logic from the evaluator,
//! following the Open/Closed Principle. New operators can be added
//! by implementing the `UnaryOperator` trait.

use crate::ir::UnaryOp;
use super::value::Value;
use super::evaluator::{EvalResult, EvalError};

// =============================================================================
// Unary Operator Trait
// =============================================================================

/// Trait for handling unary operations on values.
///
/// Implementations handle specific type combinations and operations.
pub trait UnaryOperator: Send + Sync {
    /// Check if this operator handles the given operand and operation.
    fn handles(&self, value: &Value, op: UnaryOp) -> bool;

    /// Evaluate the unary operation.
    ///
    /// Only called if `handles` returns true.
    fn evaluate(&self, value: Value, op: UnaryOp) -> EvalResult;
}

// =============================================================================
// Numeric Negation Operator
// =============================================================================

/// Unary negation for numeric types.
pub struct NegationOperator;

impl UnaryOperator for NegationOperator {
    fn handles(&self, value: &Value, op: UnaryOp) -> bool {
        matches!(
            (value, op),
            (Value::Int(_) | Value::Float(_), UnaryOp::Neg)
        )
    }

    fn evaluate(&self, value: Value, op: UnaryOp) -> EvalResult {
        match (value, op) {
            (Value::Int(n), UnaryOp::Neg) => Ok(Value::Int(-n)),
            (Value::Float(f), UnaryOp::Neg) => Ok(Value::Float(-f)),
            _ => unreachable!(),
        }
    }
}

// =============================================================================
// Logical Not Operator
// =============================================================================

/// Logical not for booleans.
pub struct LogicalNotOperator;

impl UnaryOperator for LogicalNotOperator {
    fn handles(&self, value: &Value, op: UnaryOp) -> bool {
        matches!((value, op), (Value::Bool(_), UnaryOp::Not))
    }

    fn evaluate(&self, value: Value, op: UnaryOp) -> EvalResult {
        match (value, op) {
            (Value::Bool(b), UnaryOp::Not) => Ok(Value::Bool(!b)),
            _ => unreachable!(),
        }
    }
}

// =============================================================================
// Bitwise Not Operator
// =============================================================================

/// Bitwise not for integers.
pub struct BitwiseNotOperator;

impl UnaryOperator for BitwiseNotOperator {
    fn handles(&self, value: &Value, op: UnaryOp) -> bool {
        matches!((value, op), (Value::Int(_), UnaryOp::BitNot))
    }

    fn evaluate(&self, value: Value, op: UnaryOp) -> EvalResult {
        match (value, op) {
            (Value::Int(n), UnaryOp::BitNot) => Ok(Value::Int(!n)),
            _ => unreachable!(),
        }
    }
}

// =============================================================================
// Try Operator
// =============================================================================

/// Try operator (?) for Option and Result types.
///
/// For Option types: unwraps Some, propagates None
/// For Result types: unwraps Ok, propagates Err
/// For other types: passes through unchanged (for compatibility)
pub struct TryOperator;

impl UnaryOperator for TryOperator {
    fn handles(&self, _value: &Value, op: UnaryOp) -> bool {
        // Handle Try for all value types
        matches!(op, UnaryOp::Try)
    }

    fn evaluate(&self, value: Value, _op: UnaryOp) -> EvalResult {
        match value {
            Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
            Value::Err(e) => Err(EvalError::propagate(
                Value::Err(e.clone()),
                format!("propagated error: {e:?}"),
            )),
            Value::None => {
                Err(EvalError::propagate(Value::None, "propagated None"))
            }
            // Pass through for non-Option/Result types
            other => Ok(other),
        }
    }
}

// =============================================================================
// Unary Operator Registry
// =============================================================================

/// Registry of unary operators.
///
/// Provides a way to evaluate unary operations by delegating to registered operators.
pub struct UnaryOperatorRegistry {
    operators: Vec<Box<dyn UnaryOperator>>,
}

impl UnaryOperatorRegistry {
    /// Create a new unary operator registry with all built-in operators.
    pub fn new() -> Self {
        UnaryOperatorRegistry {
            operators: vec![
                Box::new(NegationOperator),
                Box::new(LogicalNotOperator),
                Box::new(BitwiseNotOperator),
                Box::new(TryOperator),
            ],
        }
    }

    /// Evaluate a unary operation.
    ///
    /// Tries each registered operator in order until one handles the operation.
    pub fn evaluate(&self, value: Value, op: UnaryOp) -> EvalResult {
        for handler in &self.operators {
            if handler.handles(&value, op) {
                return handler.evaluate(value, op);
            }
        }
        Err(EvalError::new(format!(
            "invalid unary {:?} on {}",
            op,
            value.type_name()
        )))
    }
}

impl Default for UnaryOperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negation() {
        let registry = UnaryOperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Int(5), UnaryOp::Neg).unwrap(),
            Value::Int(-5)
        );
        assert_eq!(
            registry.evaluate(Value::Float(3.14), UnaryOp::Neg).unwrap(),
            Value::Float(-3.14)
        );
    }

    #[test]
    fn test_logical_not() {
        let registry = UnaryOperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Bool(true), UnaryOp::Not).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            registry.evaluate(Value::Bool(false), UnaryOp::Not).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_bitwise_not() {
        let registry = UnaryOperatorRegistry::new();

        assert_eq!(
            registry.evaluate(Value::Int(0), UnaryOp::BitNot).unwrap(),
            Value::Int(-1)
        );
        assert_eq!(
            registry.evaluate(Value::Int(-1), UnaryOp::BitNot).unwrap(),
            Value::Int(0)
        );
    }

    #[test]
    fn test_try_ok() {
        let registry = UnaryOperatorRegistry::new();

        let ok_val = Value::ok(Value::Int(42));
        assert_eq!(
            registry.evaluate(ok_val, UnaryOp::Try).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn test_try_err() {
        let registry = UnaryOperatorRegistry::new();

        let err_val = Value::err(Value::string("error"));
        let result = registry.evaluate(err_val, UnaryOp::Try);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_some() {
        let registry = UnaryOperatorRegistry::new();

        let some_val = Value::some(Value::Int(42));
        assert_eq!(
            registry.evaluate(some_val, UnaryOp::Try).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn test_try_none() {
        let registry = UnaryOperatorRegistry::new();

        let result = registry.evaluate(Value::None, UnaryOp::Try);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_passthrough() {
        let registry = UnaryOperatorRegistry::new();

        // Non-Option/Result types pass through unchanged
        assert_eq!(
            registry.evaluate(Value::Int(42), UnaryOp::Try).unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            registry.evaluate(Value::Bool(true), UnaryOp::Try).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_invalid_unary() {
        let registry = UnaryOperatorRegistry::new();

        // Can't negate a boolean
        let result = registry.evaluate(Value::Bool(true), UnaryOp::Neg);
        assert!(result.is_err());
    }
}
