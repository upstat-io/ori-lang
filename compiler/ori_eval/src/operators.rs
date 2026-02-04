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

// Helper functions for repetitive checked arithmetic patterns

/// Checked arithmetic operation with overflow handling.
///
/// Used for Add, Sub, Mul where the only error case is overflow.
#[inline]
fn checked_arith<T>(result: Option<T>, wrap: fn(T) -> Value, op_name: &'static str) -> EvalResult {
    result.map(wrap).ok_or_else(|| integer_overflow(op_name))
}

/// Checked division with zero guard.
///
/// Returns `division_by_zero` error if divisor is zero, `integer_overflow` if result overflows.
#[inline]
fn checked_div<T, F>(
    is_zero: bool,
    op: F,
    wrap: fn(T) -> Value,
    op_name: &'static str,
) -> EvalResult
where
    F: FnOnce() -> Option<T>,
{
    if is_zero {
        Err(division_by_zero())
    } else {
        op().map(wrap).ok_or_else(|| integer_overflow(op_name))
    }
}

/// Checked modulo with zero guard.
///
/// Returns `modulo_by_zero` error if divisor is zero, `integer_overflow` if result overflows.
#[inline]
fn checked_mod<T, F>(
    is_zero: bool,
    op: F,
    wrap: fn(T) -> Value,
    op_name: &'static str,
) -> EvalResult
where
    F: FnOnce() -> Option<T>,
{
    if is_zero {
        Err(modulo_by_zero())
    } else {
        op().map(wrap).ok_or_else(|| integer_overflow(op_name))
    }
}

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
        (Value::Duration(a), Value::Duration(b)) => eval_duration_binary(*a, *b, op),
        (Value::Duration(a), Value::Int(b)) => eval_duration_int_binary(*a, *b, op),
        (Value::Int(a), Value::Duration(b)) => eval_int_duration_binary(*a, *b, op),
        (Value::Size(a), Value::Size(b)) => eval_size_binary(*a, *b, op),
        (Value::Size(a), Value::Int(b)) => eval_size_int_binary(*a, *b, op),
        (Value::Int(a), Value::Size(b)) => eval_int_size_binary(*a, *b, op),
        (Value::Some(_) | Value::None, Value::Some(_) | Value::None) => {
            eval_option_binary(&left, &right, op)
        }
        (Value::Ok(_) | Value::Err(_), Value::Ok(_) | Value::Err(_)) => {
            eval_result_binary(&left, &right, op)
        }
        (Value::Struct(a), Value::Struct(b)) => eval_struct_binary(a, b, op),
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
        BinaryOp::Add => checked_arith(a.checked_add(b), Value::Int, "addition"),
        BinaryOp::Sub => checked_arith(a.checked_sub(b), Value::Int, "subtraction"),
        BinaryOp::Mul => checked_arith(a.checked_mul(b), Value::Int, "multiplication"),
        BinaryOp::Div => checked_div(b.is_zero(), || a.checked_div(b), Value::Int, "division"),
        BinaryOp::Mod => checked_mod(b.is_zero(), || a.checked_rem(b), Value::Int, "remainder"),
        BinaryOp::FloorDiv => checked_div(
            b.is_zero(),
            || a.checked_floor_div(b),
            Value::Int,
            "floor division",
        ),
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
fn eval_string_binary(a: &str, b: &str, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => {
            let result = format!("{a}{b}");
            Ok(Value::string(result))
        }
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        // Lexicographic comparison
        BinaryOp::Lt => Ok(Value::Bool(a < b)),
        BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
        BinaryOp::Gt => Ok(Value::Bool(a > b)),
        BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
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
///
/// Per spec: `None < Some` - None is always less than any Some value.
/// For `Some(a)` vs `Some(b)`, recursively compare inner values.
fn eval_option_binary(left: &Value, right: &Value, op: BinaryOp) -> EvalResult {
    match (left, right) {
        (Value::Some(a), Value::Some(b)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
            BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
            // Recursive comparison for Some values
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                // Compare inner values recursively
                evaluate_binary((**a).clone(), (**b).clone(), op)
            }
            _ => Err(invalid_binary_op_for("Option", op)),
        },
        (Value::None, Value::None) => match op {
            BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::GtEq => Ok(Value::Bool(true)),
            BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::Gt => Ok(Value::Bool(false)),
            _ => Err(invalid_binary_op_for("Option", op)),
        },
        (Value::None, Value::Some(_)) => match op {
            // None < Some(_) - None is always less than Some
            BinaryOp::Eq | BinaryOp::Gt | BinaryOp::GtEq => Ok(Value::Bool(false)),
            BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::LtEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Option", op)),
        },
        (Value::Some(_), Value::None) => match op {
            // Some(_) > None - Some is always greater than None
            BinaryOp::Eq | BinaryOp::Lt | BinaryOp::LtEq => Ok(Value::Bool(false)),
            BinaryOp::NotEq | BinaryOp::Gt | BinaryOp::GtEq => Ok(Value::Bool(true)),
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

/// Binary operations on Duration values (stored as i64 nanoseconds).
fn eval_duration_binary(a: i64, b: i64, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => checked_arith(a.checked_add(b), Value::Duration, "duration addition"),
        BinaryOp::Sub => checked_arith(a.checked_sub(b), Value::Duration, "duration subtraction"),
        BinaryOp::Mod => checked_mod(
            b == 0,
            || a.checked_rem(b),
            Value::Duration,
            "duration modulo",
        ),
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        BinaryOp::Lt => Ok(Value::Bool(a < b)),
        BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
        BinaryOp::Gt => Ok(Value::Bool(a > b)),
        BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
        _ => Err(invalid_binary_op_for("Duration", op)),
    }
}

/// Binary operations: Duration * int or Duration / int.
fn eval_duration_int_binary(a: i64, b: ScalarInt, op: BinaryOp) -> EvalResult {
    let b_val = b.raw();
    match op {
        BinaryOp::Mul => checked_arith(
            a.checked_mul(b_val),
            Value::Duration,
            "duration multiplication",
        ),
        BinaryOp::Div => checked_div(
            b_val == 0,
            || a.checked_div(b_val),
            Value::Duration,
            "duration division",
        ),
        _ => Err(invalid_binary_op_for("Duration and int", op)),
    }
}

/// Binary operations: int * Duration.
fn eval_int_duration_binary(a: ScalarInt, b: i64, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Mul => checked_arith(
            a.raw().checked_mul(b),
            Value::Duration,
            "duration multiplication",
        ),
        _ => Err(invalid_binary_op_for("int and Duration", op)),
    }
}

/// Binary operations on Size values (stored as u64 bytes).
fn eval_size_binary(a: u64, b: u64, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => checked_arith(a.checked_add(b), Value::Size, "size addition"),
        // Size subtraction has special error message (not "overflow" but "negative value")
        BinaryOp::Sub => a
            .checked_sub(b)
            .map(Value::Size)
            .ok_or_else(|| EvalError::new("size subtraction would result in negative value")),
        BinaryOp::Mod => checked_mod(b == 0, || a.checked_rem(b), Value::Size, "size modulo"),
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        BinaryOp::Lt => Ok(Value::Bool(a < b)),
        BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
        BinaryOp::Gt => Ok(Value::Bool(a > b)),
        BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
        _ => Err(invalid_binary_op_for("Size", op)),
    }
}

/// Binary operations: Size * int or Size / int.
fn eval_size_int_binary(a: u64, b: ScalarInt, op: BinaryOp) -> EvalResult {
    use std::cmp::Ordering;
    let b_val = b.raw();
    match op {
        BinaryOp::Mul => match b_val.cmp(&0) {
            Ordering::Less => Err(EvalError::new("cannot multiply Size by negative integer")),
            Ordering::Equal | Ordering::Greater => a
                .checked_mul(b_val.cast_unsigned())
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size multiplication")),
        },
        BinaryOp::Div => match b_val.cmp(&0) {
            Ordering::Equal => Err(division_by_zero()),
            Ordering::Less => Err(EvalError::new("cannot divide Size by negative integer")),
            Ordering::Greater => a
                .checked_div(b_val.cast_unsigned())
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size division")),
        },
        _ => Err(invalid_binary_op_for("Size and int", op)),
    }
}

/// Binary operations: int * Size.
fn eval_int_size_binary(a: ScalarInt, b: u64, op: BinaryOp) -> EvalResult {
    use std::cmp::Ordering;
    let a_val = a.raw();
    match op {
        BinaryOp::Mul => match a_val.cmp(&0) {
            Ordering::Less => Err(EvalError::new("cannot multiply Size by negative integer")),
            Ordering::Equal | Ordering::Greater => a_val
                .cast_unsigned()
                .checked_mul(b)
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size multiplication")),
        },
        _ => Err(invalid_binary_op_for("int and Size", op)),
    }
}

/// Binary operations on struct values.
///
/// Structs support equality comparison. The comparison is structural:
/// both structs must have the same type and all fields must be equal.
fn eval_struct_binary(
    a: &ori_patterns::StructValue,
    b: &ori_patterns::StructValue,
    op: BinaryOp,
) -> EvalResult {
    match op {
        BinaryOp::Eq => {
            // Must be the same type
            if a.type_name != b.type_name {
                return Ok(Value::Bool(false));
            }
            // Compare all fields structurally using Value's PartialEq
            let equal = a.fields == b.fields;
            Ok(Value::Bool(equal))
        }
        BinaryOp::NotEq => {
            // Must be the same type
            if a.type_name != b.type_name {
                return Ok(Value::Bool(true));
            }
            let equal = a.fields == b.fields;
            Ok(Value::Bool(!equal))
        }
        _ => Err(invalid_binary_op_for("struct", op)),
    }
}
