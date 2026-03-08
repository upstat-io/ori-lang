//! Binary operator implementations for Duration and Size types.
//!
//! Extracted from the main operators module to keep file sizes
//! within the 500-line limit.

use ori_ir::BinaryOp;
use ori_patterns::{
    division_by_zero, integer_overflow, invalid_binary_op_for, size_negative_divide,
    size_negative_multiply, size_would_be_negative, EvalResult, ScalarInt, Value,
};

use super::{checked_arith, checked_mod};

/// Binary operations on Duration values (stored as i64 nanoseconds).
pub(super) fn eval_duration_binary(a: i64, b: i64, op: BinaryOp) -> EvalResult {
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
        _ => Err(invalid_binary_op_for("Duration", op).into()),
    }
}

/// Binary operations: Duration * int or Duration / int.
pub(super) fn eval_duration_int_binary(a: i64, b: ScalarInt, op: BinaryOp) -> EvalResult {
    let b_val = b.raw();
    match op {
        BinaryOp::Mul => checked_arith(
            a.checked_mul(b_val),
            Value::Duration,
            "duration multiplication",
        ),
        BinaryOp::Div => {
            if b_val == 0 {
                Err(division_by_zero().into())
            } else {
                a.checked_div(b_val)
                    .map(Value::Duration)
                    .ok_or_else(|| integer_overflow("duration division").into())
            }
        }
        _ => Err(invalid_binary_op_for("Duration and int", op).into()),
    }
}

/// Binary operations: int * Duration.
pub(super) fn eval_int_duration_binary(a: ScalarInt, b: i64, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Mul => checked_arith(
            a.raw().checked_mul(b),
            Value::Duration,
            "duration multiplication",
        ),
        _ => Err(invalid_binary_op_for("int and Duration", op).into()),
    }
}

/// Binary operations on Size values (stored as u64 bytes).
pub(super) fn eval_size_binary(a: u64, b: u64, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => checked_arith(a.checked_add(b), Value::Size, "size addition"),
        BinaryOp::Sub => a
            .checked_sub(b)
            .map(Value::Size)
            .ok_or_else(|| size_would_be_negative().into()),
        BinaryOp::Mod => checked_mod(b == 0, || a.checked_rem(b), Value::Size, "size modulo"),
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        BinaryOp::Lt => Ok(Value::Bool(a < b)),
        BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
        BinaryOp::Gt => Ok(Value::Bool(a > b)),
        BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
        _ => Err(invalid_binary_op_for("Size", op).into()),
    }
}

/// Binary operations: Size * int or Size / int.
pub(super) fn eval_size_int_binary(a: u64, b: ScalarInt, op: BinaryOp) -> EvalResult {
    use std::cmp::Ordering;
    let b_val = b.raw();
    match op {
        BinaryOp::Mul => match b_val.cmp(&0) {
            Ordering::Less => Err(size_negative_multiply().into()),
            Ordering::Equal | Ordering::Greater => a
                .checked_mul(b_val.cast_unsigned())
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size multiplication").into()),
        },
        BinaryOp::Div => match b_val.cmp(&0) {
            Ordering::Equal => Err(division_by_zero().into()),
            Ordering::Less => Err(size_negative_divide().into()),
            Ordering::Greater => a
                .checked_div(b_val.cast_unsigned())
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size division").into()),
        },
        _ => Err(invalid_binary_op_for("Size and int", op).into()),
    }
}

/// Binary operations: int * Size.
pub(super) fn eval_int_size_binary(a: ScalarInt, b: u64, op: BinaryOp) -> EvalResult {
    use std::cmp::Ordering;
    let a_val = a.raw();
    match op {
        BinaryOp::Mul => match a_val.cmp(&0) {
            Ordering::Less => Err(size_negative_multiply().into()),
            Ordering::Equal | Ordering::Greater => a_val
                .cast_unsigned()
                .checked_mul(b)
                .map(Value::Size)
                .ok_or_else(|| integer_overflow("size multiplication").into()),
        },
        _ => Err(invalid_binary_op_for("int and Size", op).into()),
    }
}
