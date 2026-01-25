//! Unary operator implementations for the evaluator.
//!
//! This module extracts unary operation logic from the evaluator,
//! following the Open/Closed Principle. New operators can be added
//! by implementing the `UnaryOperator` trait.

use sigil_ir::UnaryOp;
use sigil_patterns::{EvalError, EvalResult, Value};

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
        matches!(op, UnaryOp::Try)
    }

    fn evaluate(&self, value: Value, _op: UnaryOp) -> EvalResult {
        match value {
            Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
            Value::Err(e) => Err(EvalError::propagate(
                Value::Err(e.clone()),
                format!("propagated error: {e:?}"),
            )),
            Value::None => Err(EvalError::propagate(Value::None, "propagated None")),
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

    mod negation {
        use super::*;

        #[test]
        fn int_positive() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(5), UnaryOp::Neg).unwrap(),
                Value::Int(-5)
            );
        }

        #[test]
        fn int_negative() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(-5), UnaryOp::Neg).unwrap(),
                Value::Int(5)
            );
        }

        #[test]
        fn int_zero() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(0), UnaryOp::Neg).unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn int_max() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(i64::MAX), UnaryOp::Neg).unwrap(),
                Value::Int(-i64::MAX)
            );
        }

        #[test]
        #[should_panic(expected = "negate with overflow")]
        fn int_min_overflow_panics() {
            let registry = UnaryOperatorRegistry::new();
            let _ = registry.evaluate(Value::Int(i64::MIN), UnaryOp::Neg);
        }

        #[test]
        fn float_positive() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Float(3.14), UnaryOp::Neg).unwrap(),
                Value::Float(-3.14)
            );
        }

        #[test]
        fn float_negative() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Float(-3.14), UnaryOp::Neg).unwrap(),
                Value::Float(3.14)
            );
        }

        #[test]
        fn float_zero() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::Float(0.0), UnaryOp::Neg).unwrap();
            if let Value::Float(f) = result {
                assert!(f == 0.0);
                assert!(f.is_sign_negative());
            } else {
                panic!("expected float");
            }
        }

        #[test]
        fn float_negative_zero() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::Float(-0.0), UnaryOp::Neg).unwrap();
            if let Value::Float(f) = result {
                assert!(f == 0.0);
                assert!(f.is_sign_positive());
            } else {
                panic!("expected float");
            }
        }

        #[test]
        fn float_infinity() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry
                    .evaluate(Value::Float(f64::INFINITY), UnaryOp::Neg)
                    .unwrap(),
                Value::Float(f64::NEG_INFINITY)
            );
        }

        #[test]
        fn float_neg_infinity() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry
                    .evaluate(Value::Float(f64::NEG_INFINITY), UnaryOp::Neg)
                    .unwrap(),
                Value::Float(f64::INFINITY)
            );
        }

        #[test]
        fn float_nan() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::Float(f64::NAN), UnaryOp::Neg).unwrap();
            if let Value::Float(f) = result {
                assert!(f.is_nan());
            } else {
                panic!("expected float");
            }
        }
    }

    mod logical_not {
        use super::*;

        #[test]
        fn true_becomes_false() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Bool(true), UnaryOp::Not).unwrap(),
                Value::Bool(false)
            );
        }

        #[test]
        fn false_becomes_true() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Bool(false), UnaryOp::Not).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn double_negation() {
            let registry = UnaryOperatorRegistry::new();
            let once = registry.evaluate(Value::Bool(true), UnaryOp::Not).unwrap();
            let twice = registry.evaluate(once, UnaryOp::Not).unwrap();
            assert_eq!(twice, Value::Bool(true));
        }
    }

    mod bitwise_not {
        use super::*;

        #[test]
        fn zero_becomes_minus_one() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(0), UnaryOp::BitNot).unwrap(),
                Value::Int(-1)
            );
        }

        #[test]
        fn minus_one_becomes_zero() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(-1), UnaryOp::BitNot).unwrap(),
                Value::Int(0)
            );
        }

        #[test]
        fn positive_value() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(5), UnaryOp::BitNot).unwrap(),
                Value::Int(-6)
            );
        }

        #[test]
        fn negative_value() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(-6), UnaryOp::BitNot).unwrap(),
                Value::Int(5)
            );
        }

        #[test]
        fn max_value() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry
                    .evaluate(Value::Int(i64::MAX), UnaryOp::BitNot)
                    .unwrap(),
                Value::Int(i64::MIN)
            );
        }

        #[test]
        fn min_value() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry
                    .evaluate(Value::Int(i64::MIN), UnaryOp::BitNot)
                    .unwrap(),
                Value::Int(i64::MAX)
            );
        }

        #[test]
        fn double_negation_identity() {
            let registry = UnaryOperatorRegistry::new();
            let val = Value::Int(12345);
            let once = registry.evaluate(val.clone(), UnaryOp::BitNot).unwrap();
            let twice = registry.evaluate(once, UnaryOp::BitNot).unwrap();
            assert_eq!(twice, val);
        }

        #[test]
        fn powers_of_two() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(1), UnaryOp::BitNot).unwrap(),
                Value::Int(-2)
            );
            assert_eq!(
                registry.evaluate(Value::Int(2), UnaryOp::BitNot).unwrap(),
                Value::Int(-3)
            );
            assert_eq!(
                registry.evaluate(Value::Int(4), UnaryOp::BitNot).unwrap(),
                Value::Int(-5)
            );
        }
    }

    mod try_operator {
        use super::*;

        #[test]
        fn ok_unwraps() {
            let registry = UnaryOperatorRegistry::new();
            let ok_val = Value::ok(Value::Int(42));
            assert_eq!(
                registry.evaluate(ok_val, UnaryOp::Try).unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn err_propagates() {
            let registry = UnaryOperatorRegistry::new();
            let err_val = Value::err(Value::string("error"));
            let result = registry.evaluate(err_val, UnaryOp::Try);
            assert!(result.is_err());
        }

        #[test]
        fn some_unwraps() {
            let registry = UnaryOperatorRegistry::new();
            let some_val = Value::some(Value::Int(42));
            assert_eq!(
                registry.evaluate(some_val, UnaryOp::Try).unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn none_propagates() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::None, UnaryOp::Try);
            assert!(result.is_err());
        }

        #[test]
        fn nested_ok() {
            let registry = UnaryOperatorRegistry::new();
            let nested = Value::ok(Value::ok(Value::Int(42)));
            let result = registry.evaluate(nested, UnaryOp::Try).unwrap();
            assert_eq!(result, Value::ok(Value::Int(42)));
        }

        #[test]
        fn nested_some() {
            let registry = UnaryOperatorRegistry::new();
            let nested = Value::some(Value::some(Value::Int(42)));
            let result = registry.evaluate(nested, UnaryOp::Try).unwrap();
            assert_eq!(result, Value::some(Value::Int(42)));
        }

        #[test]
        fn passthrough_int() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Int(42), UnaryOp::Try).unwrap(),
                Value::Int(42)
            );
        }

        #[test]
        fn passthrough_bool() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Bool(true), UnaryOp::Try).unwrap(),
                Value::Bool(true)
            );
        }

        #[test]
        fn passthrough_string() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry
                    .evaluate(Value::string("hello"), UnaryOp::Try)
                    .unwrap(),
                Value::string("hello")
            );
        }

        #[test]
        fn passthrough_void() {
            let registry = UnaryOperatorRegistry::new();
            assert_eq!(
                registry.evaluate(Value::Void, UnaryOp::Try).unwrap(),
                Value::Void
            );
        }
    }

    mod type_errors {
        use super::*;

        #[test]
        fn negate_bool_fails() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::Bool(true), UnaryOp::Neg);
            assert!(result.is_err());
        }

        #[test]
        fn negate_string_fails() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::string("hello"), UnaryOp::Neg);
            assert!(result.is_err());
        }

        #[test]
        fn logical_not_int_fails() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::Int(1), UnaryOp::Not);
            assert!(result.is_err());
        }

        #[test]
        fn logical_not_string_fails() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::string("hello"), UnaryOp::Not);
            assert!(result.is_err());
        }

        #[test]
        fn bitwise_not_float_fails() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::Float(3.14), UnaryOp::BitNot);
            assert!(result.is_err());
        }

        #[test]
        fn bitwise_not_bool_fails() {
            let registry = UnaryOperatorRegistry::new();
            let result = registry.evaluate(Value::Bool(true), UnaryOp::BitNot);
            assert!(result.is_err());
        }
    }
}
