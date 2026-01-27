//! Binary operator implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for binary operations. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use ori_ir::BinaryOp;
use ori_patterns::{
    binary_type_mismatch, division_by_zero, integer_overflow, invalid_binary_op, modulo_by_zero,
    EvalError, EvalResult, Heap, RangeValue, ScalarInt, Value,
};

// Direct Dispatch Function

/// Evaluate a binary operation using direct pattern matching.
///
/// This is the preferred entry point for binary operations. It uses
/// enum-based dispatch which is faster than trait objects for fixed type sets.
#[expect(clippy::needless_pass_by_value, reason = "Public API consumed by callers passing owned Values; references would force cloning at call sites")]
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
///
/// All arithmetic goes through `ScalarInt`'s checked methods — unchecked
/// overflow is impossible because `ScalarInt` does not implement `Add`,
/// `Sub`, `Mul`, `Div`, `Rem`, or `Neg`.
fn eval_int_binary(a: ScalarInt, b: ScalarInt, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => a.checked_add(b).map(Value::Int).ok_or_else(|| integer_overflow("addition")),
        BinaryOp::Sub => a.checked_sub(b).map(Value::Int).ok_or_else(|| integer_overflow("subtraction")),
        BinaryOp::Mul => a.checked_mul(b).map(Value::Int).ok_or_else(|| integer_overflow("multiplication")),
        BinaryOp::Div => {
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_div(b).map(Value::Int).ok_or_else(|| integer_overflow("division"))
            }
        }
        BinaryOp::Mod => {
            if b.is_zero() {
                Err(modulo_by_zero())
            } else {
                a.checked_rem(b).map(Value::Int).ok_or_else(|| integer_overflow("remainder"))
            }
        }
        BinaryOp::FloorDiv => {
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_floor_div(b).map(Value::Int).ok_or_else(|| integer_overflow("floor division"))
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
        BinaryOp::Shl => a.checked_shl(b).map(Value::Int).ok_or_else(|| {
            EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))
        }),
        BinaryOp::Shr => a.checked_shr(b).map(Value::Int).ok_or_else(|| {
            EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))
        }),
        BinaryOp::Range => Ok(Value::Range(RangeValue::exclusive(a.raw(), b.raw()))),
        BinaryOp::RangeInclusive => Ok(Value::Range(RangeValue::inclusive(a.raw(), b.raw()))),
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
            evaluate_binary(Value::int(2), Value::int(3), BinaryOp::Add).unwrap(),
            Value::int(5)
        );
        assert_eq!(
            evaluate_binary(Value::int(5), Value::int(3), BinaryOp::Sub).unwrap(),
            Value::int(2)
        );
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(3), BinaryOp::Mul).unwrap(),
            Value::int(6)
        );
        assert_eq!(
            evaluate_binary(Value::int(7), Value::int(2), BinaryOp::Div).unwrap(),
            Value::int(3)
        );
        assert_eq!(
            evaluate_binary(Value::int(7), Value::int(2), BinaryOp::Mod).unwrap(),
            Value::int(1)
        );
    }

    #[test]
    fn test_division_by_zero() {
        assert!(evaluate_binary(Value::int(1), Value::int(0), BinaryOp::Div).is_err());
        assert!(evaluate_binary(Value::int(1), Value::int(0), BinaryOp::Mod).is_err());
    }

    #[test]
    fn test_comparisons() {
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(3), BinaryOp::Lt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::int(3), Value::int(2), BinaryOp::Gt).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::int(2), Value::int(2), BinaryOp::Eq).unwrap(),
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
            Value::list(vec![Value::int(1)]),
            Value::list(vec![Value::int(2)]),
            BinaryOp::Add,
        )
        .unwrap();
        assert_eq!(result, Value::list(vec![Value::int(1), Value::int(2)]));
    }

    #[test]
    fn test_type_mismatch() {
        assert!(evaluate_binary(Value::int(1), Value::Bool(true), BinaryOp::Add).is_err());
    }

    #[test]
    fn test_shift_amount_validation() {
        // Valid shift
        assert_eq!(
            evaluate_binary(Value::int(1), Value::int(3), BinaryOp::Shl).unwrap(),
            Value::int(8)
        );

        // Invalid shift (negative)
        assert!(evaluate_binary(Value::int(1), Value::int(-1), BinaryOp::Shl).is_err());

        // Invalid shift (too large)
        assert!(evaluate_binary(Value::int(1), Value::int(64), BinaryOp::Shl).is_err());
    }

    #[test]
    fn test_addition_overflow() {
        let result = evaluate_binary(Value::int(i64::MAX), Value::int(1), BinaryOp::Add);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("integer overflow"));
    }

    #[test]
    fn test_subtraction_overflow() {
        let result = evaluate_binary(Value::int(i64::MIN), Value::int(1), BinaryOp::Sub);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("integer overflow"));
    }

    #[test]
    fn test_multiplication_overflow() {
        let result = evaluate_binary(Value::int(i64::MAX), Value::int(2), BinaryOp::Mul);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("integer overflow"));
    }

    #[test]
    fn test_division_overflow() {
        // i64::MIN / -1 overflows because |i64::MIN| > i64::MAX
        let result = evaluate_binary(Value::int(i64::MIN), Value::int(-1), BinaryOp::Div);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("integer overflow"));
    }

    #[test]
    fn test_option_equality() {
        assert_eq!(
            evaluate_binary(Value::some(Value::int(1)), Value::some(Value::int(1)), BinaryOp::Eq)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(Value::some(Value::int(1)), Value::None, BinaryOp::Eq).unwrap(),
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
            evaluate_binary(Value::ok(Value::int(1)), Value::ok(Value::int(1)), BinaryOp::Eq)
                .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            evaluate_binary(
                Value::ok(Value::int(1)),
                Value::err(Value::string("e")),
                BinaryOp::Eq
            )
            .unwrap(),
            Value::Bool(false)
        );
    }

    // =========================================================================
    // Overflow error message tests
    // =========================================================================

    fn assert_overflow(left: i64, right: i64, op: BinaryOp) {
        let result = evaluate_binary(Value::int(left), Value::int(right), op);
        assert!(result.is_err(), "expected overflow for {left} {op:?} {right}");
        assert!(
            result.unwrap_err().message.contains("integer overflow"),
            "expected 'integer overflow' message"
        );
    }

    #[test]
    fn overflow_add_max_plus_1() {
        assert_overflow(i64::MAX, 1, BinaryOp::Add);
    }

    #[test]
    fn overflow_add_max_plus_max() {
        assert_overflow(i64::MAX, i64::MAX, BinaryOp::Add);
    }

    #[test]
    fn overflow_add_min_plus_neg1() {
        assert_overflow(i64::MIN, -1, BinaryOp::Add);
    }

    #[test]
    fn overflow_sub_min_minus_1() {
        assert_overflow(i64::MIN, 1, BinaryOp::Sub);
    }

    #[test]
    fn overflow_sub_max_minus_neg1() {
        assert_overflow(i64::MAX, -1, BinaryOp::Sub);
    }

    #[test]
    fn overflow_mul_max_times_2() {
        assert_overflow(i64::MAX, 2, BinaryOp::Mul);
    }

    #[test]
    fn overflow_mul_min_times_neg1() {
        assert_overflow(i64::MIN, -1, BinaryOp::Mul);
    }

    #[test]
    fn overflow_mul_min_times_2() {
        assert_overflow(i64::MIN, 2, BinaryOp::Mul);
    }

    #[test]
    fn overflow_div_min_by_neg1() {
        assert_overflow(i64::MIN, -1, BinaryOp::Div);
    }

    #[test]
    fn overflow_floor_div_min_by_neg1() {
        assert_overflow(i64::MIN, -1, BinaryOp::FloorDiv);
    }

    #[test]
    fn overflow_rem_min_mod_neg1() {
        let result = evaluate_binary(Value::int(i64::MIN), Value::int(-1), BinaryOp::Mod);
        assert!(result.is_err());
    }

    // =========================================================================
    // Division/modulo by zero error tests
    // =========================================================================

    #[test]
    fn div_zero_message() {
        let result = evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Div);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("division by zero"));
    }

    #[test]
    fn mod_zero_message() {
        let result = evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Mod);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("modulo by zero"));
    }

    #[test]
    fn floor_div_zero_message() {
        let result = evaluate_binary(Value::int(42), Value::int(0), BinaryOp::FloorDiv);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("division by zero"));
    }

    // =========================================================================
    // Near-boundary valid operations (must NOT overflow)
    // =========================================================================

    #[test]
    fn near_boundary_add_valid() {
        // MAX - 1 + 1 = MAX
        assert_eq!(
            evaluate_binary(Value::int(i64::MAX - 1), Value::int(1), BinaryOp::Add).unwrap(),
            Value::int(i64::MAX)
        );
    }

    #[test]
    fn near_boundary_add_overflow() {
        // MAX - 1 + 2 overflows
        assert_overflow(i64::MAX - 1, 2, BinaryOp::Add);
    }

    #[test]
    fn near_boundary_sub_valid() {
        // MIN + 1 - 1 = MIN
        assert_eq!(
            evaluate_binary(Value::int(i64::MIN + 1), Value::int(1), BinaryOp::Sub).unwrap(),
            Value::int(i64::MIN)
        );
    }

    #[test]
    fn near_boundary_sub_overflow() {
        // MIN + 1 - 2 overflows
        assert_overflow(i64::MIN + 1, 2, BinaryOp::Sub);
    }

    #[test]
    fn near_boundary_mul_valid() {
        // (MAX / 2) * 2 should not overflow
        let half = i64::MAX / 2;
        assert_eq!(
            evaluate_binary(Value::int(half), Value::int(2), BinaryOp::Mul).unwrap(),
            Value::int(half * 2)
        );
    }

    // =========================================================================
    // Floor division edge cases
    // =========================================================================

    #[test]
    fn floor_div_positive() {
        assert_eq!(
            evaluate_binary(Value::int(7), Value::int(2), BinaryOp::FloorDiv).unwrap(),
            Value::int(3)
        );
    }

    #[test]
    fn floor_div_negative_numerator() {
        assert_eq!(
            evaluate_binary(Value::int(-7), Value::int(2), BinaryOp::FloorDiv).unwrap(),
            Value::int(-4)
        );
    }

    #[test]
    fn floor_div_negative_denominator() {
        assert_eq!(
            evaluate_binary(Value::int(7), Value::int(-2), BinaryOp::FloorDiv).unwrap(),
            Value::int(-4)
        );
    }

    #[test]
    fn floor_div_both_negative() {
        assert_eq!(
            evaluate_binary(Value::int(-7), Value::int(-2), BinaryOp::FloorDiv).unwrap(),
            Value::int(3)
        );
    }

    // =========================================================================
    // Remainder sign semantics
    // =========================================================================

    #[test]
    fn rem_negative_numerator() {
        // -7 % 3 = -1 (sign follows numerator)
        assert_eq!(
            evaluate_binary(Value::int(-7), Value::int(3), BinaryOp::Mod).unwrap(),
            Value::int(-1)
        );
    }

    #[test]
    fn rem_negative_denominator() {
        // 7 % -3 = 1 (sign follows numerator)
        assert_eq!(
            evaluate_binary(Value::int(7), Value::int(-3), BinaryOp::Mod).unwrap(),
            Value::int(1)
        );
    }

    #[test]
    fn rem_both_negative() {
        // -7 % -3 = -1
        assert_eq!(
            evaluate_binary(Value::int(-7), Value::int(-3), BinaryOp::Mod).unwrap(),
            Value::int(-1)
        );
    }

    // =========================================================================
    // Shift amount validation
    // =========================================================================

    #[test]
    fn shl_boundary_63() {
        assert_eq!(
            evaluate_binary(Value::int(1), Value::int(63), BinaryOp::Shl).unwrap(),
            Value::int(i64::MIN) // sign bit set
        );
    }

    #[test]
    fn shr_boundary_63() {
        assert_eq!(
            evaluate_binary(Value::int(i64::MIN), Value::int(63), BinaryOp::Shr).unwrap(),
            Value::int(-1)
        );
    }

    #[test]
    fn shl_zero_shift() {
        assert_eq!(
            evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Shl).unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn shr_zero_shift() {
        assert_eq!(
            evaluate_binary(Value::int(42), Value::int(0), BinaryOp::Shr).unwrap(),
            Value::int(42)
        );
    }

    #[test]
    fn shl_64_out_of_range() {
        let result = evaluate_binary(Value::int(1), Value::int(64), BinaryOp::Shl);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("out of range"));
    }

    #[test]
    fn shr_64_out_of_range() {
        let result = evaluate_binary(Value::int(1), Value::int(64), BinaryOp::Shr);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("out of range"));
    }

    #[test]
    fn shl_negative_out_of_range() {
        let result = evaluate_binary(Value::int(1), Value::int(-1), BinaryOp::Shl);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("out of range"));
    }

    #[test]
    fn shr_negative_out_of_range() {
        let result = evaluate_binary(Value::int(1), Value::int(-1), BinaryOp::Shr);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("out of range"));
    }
}
