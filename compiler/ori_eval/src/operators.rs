//! Binary operator implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for binary operations. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.

use ori_ir::BinaryOp;
use ori_patterns::{
    binary_type_mismatch, division_by_zero, integer_overflow, invalid_binary_op_for,
    modulo_by_zero, EvalError, EvalResult, Heap, RangeValue, ScalarInt, Value,
};

// Direct Dispatch Function

/// Evaluate a binary operation using direct pattern matching.
///
/// This is the preferred entry point for binary operations. It uses
/// enum-based dispatch which is faster than trait objects for fixed type sets.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Public API consumed by callers passing owned Values; references would force cloning at call sites"
)]
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
        BinaryOp::Add => a
            .checked_add(b)
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("addition")),
        BinaryOp::Sub => a
            .checked_sub(b)
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("subtraction")),
        BinaryOp::Mul => a
            .checked_mul(b)
            .map(Value::Int)
            .ok_or_else(|| integer_overflow("multiplication")),
        BinaryOp::Div => {
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_div(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("division"))
            }
        }
        BinaryOp::Mod => {
            if b.is_zero() {
                Err(modulo_by_zero())
            } else {
                a.checked_rem(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("remainder"))
            }
        }
        BinaryOp::FloorDiv => {
            if b.is_zero() {
                Err(division_by_zero())
            } else {
                a.checked_floor_div(b)
                    .map(Value::Int)
                    .ok_or_else(|| integer_overflow("floor division"))
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
        BinaryOp::Shl => a
            .checked_shl(b)
            .map(Value::Int)
            .ok_or_else(|| EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))),
        BinaryOp::Shr => a
            .checked_shr(b)
            .map(Value::Int)
            .ok_or_else(|| EvalError::new(format!("shift amount {} out of range (0-63)", b.raw()))),
        BinaryOp::Range => Ok(Value::Range(RangeValue::exclusive(a.raw(), b.raw()))),
        BinaryOp::RangeInclusive => Ok(Value::Range(RangeValue::inclusive(a.raw(), b.raw()))),
        _ => Err(invalid_binary_op_for("integers", op)),
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
        _ => Err(invalid_binary_op_for("floats", op)),
    }
}

/// Binary operations on booleans.
fn eval_bool_binary(a: bool, b: bool, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        _ => Err(invalid_binary_op_for("booleans", op)),
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
        _ => Err(invalid_binary_op_for("strings", op)),
    }
}

/// Binary operations on lists.
fn eval_list_binary(a: &Heap<Vec<Value>>, b: &Heap<Vec<Value>>, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => {
            let mut result = (**a).clone();
            result.extend_from_slice(b);
            Ok(Value::list(result))
        }
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        _ => Err(invalid_binary_op_for("lists", op)),
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
        _ => Err(invalid_binary_op_for("char", op)),
    }
}

/// Binary operations on tuples.
fn eval_tuple_binary(a: &Heap<Vec<Value>>, b: &Heap<Vec<Value>>, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        _ => Err(invalid_binary_op_for("tuples", op)),
    }
}

/// Binary operations on Option values.
fn eval_option_binary(left: &Value, right: &Value, op: BinaryOp) -> EvalResult {
    match (left, right) {
        (Value::Some(a), Value::Some(b)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
            BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
            _ => Err(invalid_binary_op_for("Option", op)),
        },
        (Value::None, Value::None) => match op {
            BinaryOp::Eq => Ok(Value::Bool(true)),
            BinaryOp::NotEq => Ok(Value::Bool(false)),
            _ => Err(invalid_binary_op_for("Option", op)),
        },
        (Value::Some(_), Value::None) | (Value::None, Value::Some(_)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(false)),
            BinaryOp::NotEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Option", op)),
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
            _ => Err(invalid_binary_op_for("Result", op)),
        },
        (Value::Ok(_), Value::Err(_)) | (Value::Err(_), Value::Ok(_)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(false)),
            BinaryOp::NotEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Result", op)),
        },
        _ => unreachable!(),
    }
}
