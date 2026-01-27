//! Binary operator implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for binary operations. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use ori_ir::BinaryOp;
use ori_patterns::{
    binary_type_mismatch, division_by_zero, invalid_binary_op, modulo_by_zero, EvalError,
    EvalResult, Heap, RangeValue, Value,
};

// Direct Dispatch Function

/// Evaluate a binary operation using direct pattern matching.
///
/// This is the preferred entry point for binary operations. It uses
/// enum-based dispatch which is faster than trait objects for fixed type sets.
pub fn evaluate_binary(left: Value, right: Value, op: BinaryOp) -> EvalResult {
    match (&left, &right) {
        (Value::Int(a), Value::Int(b)) => eval_int_binary(*a, *b, op),
        (Value::Float(a), Value::Float(b)) => eval_float_binary(*a, *b, op),
        (Value::Bool(a), Value::Bool(b)) => eval_bool_binary(*a, *b, op),
        (Value::Str(a), Value::Str(b)) => eval_string_binary(a, b, op),
        (Value::List(a), Value::List(b)) => eval_list_binary(a, b, op),
        (Value::Char(a), Value::Char(b)) => eval_char_binary(*a, *b, op),
        (Value::Tuple(a), Value::Tuple(b)) => eval_tuple_binary(a, b, op),
        (Value::Some(_) | Value::None, Value::Some(_) | Value::None) => {
            eval_option_binary(&left, &right, op)
        }
        (Value::Ok(_) | Value::Err(_), Value::Ok(_) | Value::Err(_)) => {
            eval_result_binary(&left, &right, op)
        }
        _ => Err(binary_type_mismatch(left.type_name(), right.type_name())),
    }
}

// Type-Specific Evaluation Functions

/// Binary operations on integers.
fn eval_int_binary(a: i64, b: i64, op: BinaryOp) -> EvalResult {
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

/// Binary operations on floats.
fn eval_float_binary(a: f64, b: f64, op: BinaryOp) -> EvalResult {
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

/// Binary operations on booleans.
fn eval_bool_binary(a: bool, b: bool, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        _ => Err(invalid_binary_op("booleans")),
    }
}

/// Binary operations on strings.
fn eval_string_binary(a: &Heap<String>, b: &Heap<String>, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => {
            let result = format!("{}{}", &**a, &**b);
            Ok(Value::string(result))
        }
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        // Lexicographic comparison
        BinaryOp::Lt => Ok(Value::Bool(**a < **b)),
        BinaryOp::LtEq => Ok(Value::Bool(**a <= **b)),
        BinaryOp::Gt => Ok(Value::Bool(**a > **b)),
        BinaryOp::GtEq => Ok(Value::Bool(**a >= **b)),
        _ => Err(invalid_binary_op("strings")),
    }
}

/// Binary operations on lists.
fn eval_list_binary(a: &Heap<Vec<Value>>, b: &Heap<Vec<Value>>, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => {
            let mut result = (**a).clone();
            result.extend((**b).iter().cloned());
            Ok(Value::list(result))
        }
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        _ => Err(invalid_binary_op("lists")),
    }
}

/// Binary operations on characters.
fn eval_char_binary(a: char, b: char, op: BinaryOp) -> EvalResult {
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

/// Binary operations on tuples.
fn eval_tuple_binary(a: &Heap<Vec<Value>>, b: &Heap<Vec<Value>>, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        _ => Err(invalid_binary_op("tuples")),
    }
}

/// Binary operations on Option values.
fn eval_option_binary(left: &Value, right: &Value, op: BinaryOp) -> EvalResult {
    match (left, right) {
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

/// Binary operations on Result values.
fn eval_result_binary(left: &Value, right: &Value, op: BinaryOp) -> EvalResult {
    match (left, right) {
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

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn test_int_operations() {
        assert_eq!(
            evaluate_binary(Value::Int(2), Value::Int(3), BinaryOp::Add).unwrap(),
            Value::Int(5)
        );
        assert_eq!(
            evaluate_binary(Value::Int(5), Value::Int(3), BinaryOp::Sub).unwrap(),
            Value::Int(2)
        );
        assert_eq!(
            evaluate_binary(Value::Int(2), Value::Int(3), BinaryOp::Mul).unwrap(),
            Value::Int(6)
        );
        assert_eq!(
            evaluate_binary(Value::Int(7), Value::Int(2), BinaryOp::Div).unwrap(),
            Value::Int(3)
        );
        assert_eq!(
            evaluate_binary(Value::Int(7), Value::Int(2), BinaryOp::Mod).unwrap(),
            Value::Int(1)
        );
    }

    #[test]
    fn test_division_by_zero() {
        assert!(evaluate_binary(Value::Int(1), Value::Int(0), BinaryOp::Div).is_err());
        assert!(evaluate_binary(Value::Int(1), Value::Int(0), BinaryOp::Mod).is_err());
    }

    #[test]
    fn test_comparisons() {
        assert_eq!(
            evaluate_binary(Value::Int(2), Value::Int(3), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::Int(3), Value::Int(2), BinaryOp::Gt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::Int(2), Value::Int(2), BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_string_concatenation() {
        let result = evaluate_binary(
            Value::string("hello".to_string()),
            Value::string(" world".to_string()),
            BinaryOp::Add,
        )
        .unwrap();
        assert_eq!(result, Value::string("hello world".to_string()));
    }

    #[test]
    fn test_list_concatenation() {
        let result = evaluate_binary(
            Value::list(vec![Value::Int(1)]),
            Value::list(vec![Value::Int(2)]),
            BinaryOp::Add,
        )
        .unwrap();
        assert_eq!(result, Value::list(vec![Value::Int(1), Value::Int(2)]));
    }

    #[test]
    fn test_type_mismatch() {
        assert!(evaluate_binary(Value::Int(1), Value::Bool(true), BinaryOp::Add).is_err());
    }

    #[test]
    fn test_shift_amount_validation() {
        // Valid shift
        assert_eq!(
            evaluate_binary(Value::Int(1), Value::Int(3), BinaryOp::Shl).unwrap(),
            Value::Int(8)
        );

        // Invalid shift (negative)
        assert!(evaluate_binary(Value::Int(1), Value::Int(-1), BinaryOp::Shl).is_err());

        // Invalid shift (too large)
        assert!(evaluate_binary(Value::Int(1), Value::Int(64), BinaryOp::Shl).is_err());
    }

    #[test]
    fn test_option_equality() {
        assert_eq!(
            evaluate_binary(Value::some(Value::Int(1)), Value::some(Value::Int(1)), BinaryOp::Eq)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::some(Value::Int(1)), Value::None, BinaryOp::Eq).unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            evaluate_binary(Value::None, Value::None, BinaryOp::Eq).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_result_equality() {
        assert_eq!(
            evaluate_binary(Value::ok(Value::Int(1)), Value::ok(Value::Int(1)), BinaryOp::Eq)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(
                Value::ok(Value::Int(1)),
                Value::err(Value::string("e")),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(false)
        );
    }
}
