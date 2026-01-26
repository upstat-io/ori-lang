//! Binary operator implementations for the evaluator.
//!
//! This module extracts binary operation logic from the evaluator,
//! following the Open/Closed Principle. New operators can be added
//! by implementing the `BinaryOperator` trait.

use sigil_ir::BinaryOp;
use sigil_patterns::{
    binary_type_mismatch, division_by_zero, invalid_binary_op, modulo_by_zero, EvalError,
    EvalResult, RangeValue, Value,
};

// =============================================================================
// Binary Operator Trait
// =============================================================================

/// Trait for handling binary operations on values.
///
/// Implementations handle specific type combinations and operations.
pub trait BinaryOperator: Send + Sync {
    /// Check if this operator handles the given operands and operation.
    fn handles(&self, left: &Value, right: &Value, op: BinaryOp) -> bool;

    /// Evaluate the binary operation.
    ///
    /// Only called if `handles` returns true.
    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult;
}

// =============================================================================
// Integer Operator
// =============================================================================

/// Binary operations on integers.
pub struct IntOperator;

impl BinaryOperator for IntOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!((left, right), (Value::Int(_), Value::Int(_)))
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        let (Value::Int(a), Value::Int(b)) = (left, right) else {
            unreachable!()
        };

        match op {
            BinaryOp::Add => Ok(Value::Int(a + b)),
            BinaryOp::Sub => Ok(Value::Int(a - b)),
            BinaryOp::Mul => Ok(Value::Int(a * b)),
            BinaryOp::Div => {
                if b == 0 {
                    Err(division_by_zero())
                } else {
                    Ok(Value::Int(a / b))
                }
            }
            BinaryOp::Mod => {
                if b == 0 {
                    Err(modulo_by_zero())
                } else {
                    Ok(Value::Int(a % b))
                }
            }
            BinaryOp::FloorDiv => {
                if b == 0 {
                    Err(division_by_zero())
                } else {
                    let result = a / b;
                    let remainder = a % b;
                    if remainder != 0 && (a < 0) != (b < 0) {
                        Ok(Value::Int(result - 1))
                    } else {
                        Ok(Value::Int(result))
                    }
                }
            }
            BinaryOp::Eq => Ok(Value::Bool(a == b)),
            BinaryOp::NotEq => Ok(Value::Bool(a != b)),
            BinaryOp::Lt => Ok(Value::Bool(a < b)),
            BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
            BinaryOp::Gt => Ok(Value::Bool(a > b)),
            BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
            BinaryOp::BitAnd => Ok(Value::Int(a & b)),
            BinaryOp::BitOr => Ok(Value::Int(a | b)),
            BinaryOp::BitXor => Ok(Value::Int(a ^ b)),
            BinaryOp::Shl => {
                if !(0..64).contains(&b) {
                    return Err(EvalError::new(format!(
                        "shift amount {b} out of range (0-63)"
                    )));
                }
                Ok(Value::Int(a << b))
            }
            BinaryOp::Shr => {
                if !(0..64).contains(&b) {
                    return Err(EvalError::new(format!(
                        "shift amount {b} out of range (0-63)"
                    )));
                }
                Ok(Value::Int(a >> b))
            }
            BinaryOp::Range => Ok(Value::Range(RangeValue::exclusive(a, b))),
            BinaryOp::RangeInclusive => Ok(Value::Range(RangeValue::inclusive(a, b))),
            _ => Err(invalid_binary_op("integers")),
        }
    }
}

// =============================================================================
// Float Operator
// =============================================================================

/// Binary operations on floats.
pub struct FloatOperator;

impl BinaryOperator for FloatOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!((left, right), (Value::Float(_), Value::Float(_)))
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        let (Value::Float(a), Value::Float(b)) = (left, right) else {
            unreachable!()
        };

        match op {
            BinaryOp::Add => Ok(Value::Float(a + b)),
            BinaryOp::Sub => Ok(Value::Float(a - b)),
            BinaryOp::Mul => Ok(Value::Float(a * b)),
            BinaryOp::Div => Ok(Value::Float(a / b)),
            // Use partial_cmp for IEEE 754 compliant comparisons
            // (NaN != NaN, -0.0 == 0.0)
            BinaryOp::Eq => Ok(Value::Bool(
                a.partial_cmp(&b) == Some(std::cmp::Ordering::Equal),
            )),
            BinaryOp::NotEq => Ok(Value::Bool(
                a.partial_cmp(&b) != Some(std::cmp::Ordering::Equal),
            )),
            BinaryOp::Lt => Ok(Value::Bool(
                a.partial_cmp(&b) == Some(std::cmp::Ordering::Less),
            )),
            BinaryOp::LtEq => Ok(Value::Bool(matches!(
                a.partial_cmp(&b),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            ))),
            BinaryOp::Gt => Ok(Value::Bool(
                a.partial_cmp(&b) == Some(std::cmp::Ordering::Greater),
            )),
            BinaryOp::GtEq => Ok(Value::Bool(matches!(
                a.partial_cmp(&b),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            ))),
            _ => Err(invalid_binary_op("floats")),
        }
    }
}

// =============================================================================
// Boolean Operator
// =============================================================================

/// Binary operations on booleans.
pub struct BoolOperator;

impl BinaryOperator for BoolOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!((left, right), (Value::Bool(_), Value::Bool(_)))
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        let (Value::Bool(a), Value::Bool(b)) = (left, right) else {
            unreachable!()
        };

        match op {
            BinaryOp::Eq => Ok(Value::Bool(a == b)),
            BinaryOp::NotEq => Ok(Value::Bool(a != b)),
            _ => Err(invalid_binary_op("booleans")),
        }
    }
}

// =============================================================================
// String Operator
// =============================================================================

/// Binary operations on strings.
pub struct StringOperator;

impl BinaryOperator for StringOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!((left, right), (Value::Str(_), Value::Str(_)))
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        let (Value::Str(a), Value::Str(b)) = (left, right) else {
            unreachable!()
        };

        match op {
            BinaryOp::Add => {
                let result = format!("{}{}", &*a, &*b);
                Ok(Value::string(result))
            }
            BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
            BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
            // Lexicographic comparison
            BinaryOp::Lt => Ok(Value::Bool(*a < *b)),
            BinaryOp::LtEq => Ok(Value::Bool(*a <= *b)),
            BinaryOp::Gt => Ok(Value::Bool(*a > *b)),
            BinaryOp::GtEq => Ok(Value::Bool(*a >= *b)),
            _ => Err(invalid_binary_op("strings")),
        }
    }
}

// =============================================================================
// List Operator
// =============================================================================

/// Binary operations on lists.
pub struct ListOperator;

impl BinaryOperator for ListOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!((left, right), (Value::List(_), Value::List(_)))
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        let (Value::List(a), Value::List(b)) = (left, right) else {
            unreachable!()
        };

        match op {
            BinaryOp::Add => {
                let mut result = (*a).clone();
                result.extend((*b).iter().cloned());
                Ok(Value::list(result))
            }
            BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
            BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
            _ => Err(invalid_binary_op("lists")),
        }
    }
}

// =============================================================================
// Char Operator
// =============================================================================

/// Binary operations on characters.
pub struct CharOperator;

impl BinaryOperator for CharOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!((left, right), (Value::Char(_), Value::Char(_)))
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        let (Value::Char(a), Value::Char(b)) = (left, right) else {
            unreachable!()
        };

        match op {
            BinaryOp::Eq => Ok(Value::Bool(a == b)),
            BinaryOp::NotEq => Ok(Value::Bool(a != b)),
            BinaryOp::Lt => Ok(Value::Bool(a < b)),
            BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
            BinaryOp::Gt => Ok(Value::Bool(a > b)),
            BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
            _ => Err(invalid_binary_op("char")),
        }
    }
}

// =============================================================================
// Tuple Operator
// =============================================================================

/// Binary operations on tuples.
pub struct TupleOperator;

impl BinaryOperator for TupleOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!((left, right), (Value::Tuple(_), Value::Tuple(_)))
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        let (Value::Tuple(a), Value::Tuple(b)) = (left, right) else {
            unreachable!()
        };

        match op {
            BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
            BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
            _ => Err(invalid_binary_op("tuples")),
        }
    }
}

// =============================================================================
// Option Operator
// =============================================================================

/// Binary operations on Option values.
pub struct OptionOperator;

impl BinaryOperator for OptionOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!(
            (left, right),
            (Value::Some(_) | Value::None, Value::Some(_) | Value::None)
        )
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        match (&left, &right) {
            (Value::Some(a), Value::Some(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
                _ => Err(invalid_binary_op("Option")),
            },
            (Value::None, Value::None) => match op {
                BinaryOp::Eq => Ok(Value::Bool(true)),
                BinaryOp::NotEq => Ok(Value::Bool(false)),
                _ => Err(invalid_binary_op("Option")),
            },
            (Value::Some(_), Value::None) | (Value::None, Value::Some(_)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(false)),
                BinaryOp::NotEq => Ok(Value::Bool(true)),
                _ => Err(invalid_binary_op("Option")),
            },
            _ => unreachable!(),
        }
    }
}

// =============================================================================
// Result Operator
// =============================================================================

/// Binary operations on Result values.
pub struct ResultOperator;

impl BinaryOperator for ResultOperator {
    fn handles(&self, left: &Value, right: &Value, _op: BinaryOp) -> bool {
        matches!(
            (left, right),
            (Value::Ok(_) | Value::Err(_), Value::Ok(_) | Value::Err(_))
        )
    }

    fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        match (&left, &right) {
            (Value::Ok(a), Value::Ok(b)) | (Value::Err(a), Value::Err(b)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
                BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
                _ => Err(invalid_binary_op("Result")),
            },
            (Value::Ok(_), Value::Err(_)) | (Value::Err(_), Value::Ok(_)) => match op {
                BinaryOp::Eq => Ok(Value::Bool(false)),
                BinaryOp::NotEq => Ok(Value::Bool(true)),
                _ => Err(invalid_binary_op("Result")),
            },
            _ => unreachable!(),
        }
    }
}

// =============================================================================
// Operator Registry
// =============================================================================

/// Registry of binary operators.
///
/// Provides a way to evaluate binary operations by delegating to registered operators.
pub struct OperatorRegistry {
    operators: Vec<Box<dyn BinaryOperator>>,
}

impl OperatorRegistry {
    /// Create a new operator registry with all built-in operators.
    pub fn new() -> Self {
        OperatorRegistry {
            operators: vec![
                Box::new(IntOperator),
                Box::new(FloatOperator),
                Box::new(BoolOperator),
                Box::new(StringOperator),
                Box::new(ListOperator),
                Box::new(CharOperator),
                Box::new(TupleOperator),
                Box::new(OptionOperator),
                Box::new(ResultOperator),
            ],
        }
    }

    /// Evaluate a binary operation.
    ///
    /// Tries each registered operator in order until one handles the operation.
    pub fn evaluate(&self, left: Value, right: Value, op: BinaryOp) -> EvalResult {
        for handler in &self.operators {
            if handler.handles(&left, &right, op) {
                return handler.evaluate(left, right, op);
            }
        }
        Err(binary_type_mismatch(left.type_name(), right.type_name()))
    }
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_int_operations() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry
                .evaluate(Value::Int(2), Value::Int(3), BinaryOp::Add)
                .unwrap(),
            Value::Int(5)
        );
        assert_eq!(
            registry
                .evaluate(Value::Int(5), Value::Int(3), BinaryOp::Sub)
                .unwrap(),
            Value::Int(2)
        );
        assert_eq!(
            registry
                .evaluate(Value::Int(2), Value::Int(3), BinaryOp::Mul)
                .unwrap(),
            Value::Int(6)
        );
        assert_eq!(
            registry
                .evaluate(Value::Int(7), Value::Int(2), BinaryOp::Div)
                .unwrap(),
            Value::Int(3)
        );
        assert_eq!(
            registry
                .evaluate(Value::Int(7), Value::Int(2), BinaryOp::Mod)
                .unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn test_division_by_zero() {
        let registry = OperatorRegistry::new();

        assert!(registry
            .evaluate(Value::Int(1), Value::Int(0), BinaryOp::Div)
            .is_err());
        assert!(registry
            .evaluate(Value::Int(1), Value::Int(0), BinaryOp::Mod)
            .is_err());
    }

    #[test]
    fn test_comparisons() {
        let registry = OperatorRegistry::new();

        assert_eq!(
            registry
                .evaluate(Value::Int(2), Value::Int(3), BinaryOp::Lt)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry
                .evaluate(Value::Int(3), Value::Int(2), BinaryOp::Gt)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            registry
                .evaluate(Value::Int(2), Value::Int(2), BinaryOp::Eq)
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_string_concatenation() {
        let registry = OperatorRegistry::new();

        let result = registry
            .evaluate(
                Value::string("hello".to_string()),
                Value::string(" world".to_string()),
                BinaryOp::Add,
            )
            .unwrap();
        assert_eq!(result, Value::string("hello world".to_string()));
    }

    #[test]
    fn test_list_concatenation() {
        let registry = OperatorRegistry::new();

        let result = registry
            .evaluate(
                Value::list(vec![Value::Int(1)]),
                Value::list(vec![Value::Int(2)]),
                BinaryOp::Add,
            )
            .unwrap();
        assert_eq!(result, Value::list(vec![Value::Int(1), Value::Int(2)]));
    }

    #[test]
    fn test_type_mismatch() {
        let registry = OperatorRegistry::new();

        assert!(registry
            .evaluate(Value::Int(1), Value::Bool(true), BinaryOp::Add)
            .is_err());
    }

    #[test]
    fn test_shift_amount_validation() {
        let registry = OperatorRegistry::new();

        // Valid shift
        assert_eq!(
            registry
                .evaluate(Value::Int(1), Value::Int(3), BinaryOp::Shl)
                .unwrap(),
            Value::Int(8)
        );

        // Invalid shift (negative)
        assert!(registry
            .evaluate(Value::Int(1), Value::Int(-1), BinaryOp::Shl)
            .is_err());

        // Invalid shift (too large)
        assert!(registry
            .evaluate(Value::Int(1), Value::Int(64), BinaryOp::Shl)
            .is_err());
    }
}
