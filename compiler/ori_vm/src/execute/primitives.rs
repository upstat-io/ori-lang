//! Primitive numeric and logical operation semantics.

use ori_ir::{BinaryOp, UnaryOp};

use crate::{bytecode::IntBinaryOp, ExecutionError, ValueKind};

use super::value::VmValue;

pub(super) fn binary(
    operation: BinaryOp,
    left: VmValue,
    right: VmValue,
) -> Result<VmValue, ExecutionError> {
    if left.kind() == ValueKind::Float || right.kind() == ValueKind::Float {
        return float_binary(operation, left.as_float()?, right.as_float()?);
    }
    match operation {
        BinaryOp::Add => checked_int(left, right, "addition", i64::checked_add),
        BinaryOp::Sub => checked_int(left, right, "subtraction", i64::checked_sub),
        BinaryOp::Mul => checked_int(left, right, "multiplication", i64::checked_mul),
        BinaryOp::Div => checked_int(left, right, "division", i64::checked_div),
        BinaryOp::Mod => checked_int(left, right, "remainder", i64::checked_rem),
        BinaryOp::FloorDiv => checked_int(left, right, "floor division", i64::checked_div_euclid),
        BinaryOp::Eq => Ok(VmValue::bool(left == right)),
        BinaryOp::NotEq => Ok(VmValue::bool(left != right)),
        BinaryOp::Lt => compare_int(left, right, |lhs, rhs| lhs < rhs),
        BinaryOp::LtEq => compare_int(left, right, |lhs, rhs| lhs <= rhs),
        BinaryOp::Gt => compare_int(left, right, |lhs, rhs| lhs > rhs),
        BinaryOp::GtEq => compare_int(left, right, |lhs, rhs| lhs >= rhs),
        BinaryOp::And => Ok(VmValue::bool(left.as_bool()? && right.as_bool()?)),
        BinaryOp::Or => Ok(VmValue::bool(left.as_bool()? || right.as_bool()?)),
        BinaryOp::BitAnd => Ok(VmValue::int(left.as_int()? & right.as_int()?)),
        BinaryOp::BitOr => Ok(VmValue::int(left.as_int()? | right.as_int()?)),
        BinaryOp::BitXor => Ok(VmValue::int(left.as_int()? ^ right.as_int()?)),
        BinaryOp::Shl => shift(left, right, true),
        BinaryOp::Shr => shift(left, right, false),
        BinaryOp::MatMul | BinaryOp::Range | BinaryOp::RangeInclusive | BinaryOp::Coalesce => {
            Err(ExecutionError::UnsupportedPrimitive {
                operation: "binary operation",
            })
        }
    }
}

pub(super) fn unary(operation: UnaryOp, value: VmValue) -> Result<VmValue, ExecutionError> {
    match operation {
        UnaryOp::Neg if value.kind() == ValueKind::Float => {
            Ok(VmValue::float((-value.as_float()?).to_bits()))
        }
        UnaryOp::Neg => value.as_int()?.checked_neg().map(VmValue::int).ok_or(
            ExecutionError::IntegerOperation {
                operation: "negation",
            },
        ),
        UnaryOp::Not => Ok(VmValue::bool(!value.as_bool()?)),
        UnaryOp::BitNot => Ok(VmValue::int(!value.as_int()?)),
        UnaryOp::Try => Err(ExecutionError::UnsupportedPrimitive {
            operation: "unlowered try",
        }),
    }
}

pub(super) fn int_binary(
    operation: IntBinaryOp,
    left: VmValue,
    right: VmValue,
) -> Result<VmValue, ExecutionError> {
    match operation {
        IntBinaryOp::Add => checked_int(left, right, "addition", i64::checked_add),
        IntBinaryOp::Sub => checked_int(left, right, "subtraction", i64::checked_sub),
        IntBinaryOp::Mul => checked_int(left, right, "multiplication", i64::checked_mul),
        IntBinaryOp::Div => checked_int(left, right, "division", i64::checked_div),
        IntBinaryOp::Mod => checked_int(left, right, "remainder", i64::checked_rem),
        IntBinaryOp::FloorDiv => {
            checked_int(left, right, "floor division", i64::checked_div_euclid)
        }
        IntBinaryOp::Eq => compare_int(left, right, |lhs, rhs| lhs == rhs),
        IntBinaryOp::NotEq => compare_int(left, right, |lhs, rhs| lhs != rhs),
        IntBinaryOp::Lt => compare_int(left, right, |lhs, rhs| lhs < rhs),
        IntBinaryOp::LtEq => compare_int(left, right, |lhs, rhs| lhs <= rhs),
        IntBinaryOp::Gt => compare_int(left, right, |lhs, rhs| lhs > rhs),
        IntBinaryOp::GtEq => compare_int(left, right, |lhs, rhs| lhs >= rhs),
        IntBinaryOp::BitAnd => Ok(VmValue::int(left.as_int()? & right.as_int()?)),
        IntBinaryOp::BitOr => Ok(VmValue::int(left.as_int()? | right.as_int()?)),
        IntBinaryOp::BitXor => Ok(VmValue::int(left.as_int()? ^ right.as_int()?)),
        IntBinaryOp::Shl => shift(left, right, true),
        IntBinaryOp::Shr => shift(left, right, false),
    }
}

fn checked_int(
    left: VmValue,
    right: VmValue,
    operation: &'static str,
    apply: fn(i64, i64) -> Option<i64>,
) -> Result<VmValue, ExecutionError> {
    apply(left.as_int()?, right.as_int()?)
        .map(VmValue::int)
        .ok_or(ExecutionError::IntegerOperation { operation })
}

fn compare_int(
    left: VmValue,
    right: VmValue,
    compare: fn(i64, i64) -> bool,
) -> Result<VmValue, ExecutionError> {
    Ok(VmValue::bool(compare(left.as_int()?, right.as_int()?)))
}

fn shift(left: VmValue, right: VmValue, shift_left: bool) -> Result<VmValue, ExecutionError> {
    let amount = u32::try_from(right.as_int()?).map_err(|_| ExecutionError::IntegerOperation {
        operation: "negative shift",
    })?;
    let result = if shift_left {
        left.as_int()?.checked_shl(amount)
    } else {
        left.as_int()?.checked_shr(amount)
    };
    result
        .map(VmValue::int)
        .ok_or(ExecutionError::IntegerOperation {
            operation: "out-of-range shift",
        })
}

fn float_binary(operation: BinaryOp, left: f64, right: f64) -> Result<VmValue, ExecutionError> {
    match operation {
        BinaryOp::Add => Ok(VmValue::float((left + right).to_bits())),
        BinaryOp::Sub => Ok(VmValue::float((left - right).to_bits())),
        BinaryOp::Mul => Ok(VmValue::float((left * right).to_bits())),
        BinaryOp::Div => Ok(VmValue::float((left / right).to_bits())),
        BinaryOp::Eq => Ok(VmValue::bool(left.to_bits() == right.to_bits())),
        BinaryOp::NotEq => Ok(VmValue::bool(left.to_bits() != right.to_bits())),
        BinaryOp::Lt => Ok(VmValue::bool(left < right)),
        BinaryOp::LtEq => Ok(VmValue::bool(left <= right)),
        BinaryOp::Gt => Ok(VmValue::bool(left > right)),
        BinaryOp::GtEq => Ok(VmValue::bool(left >= right)),
        _ => Err(ExecutionError::UnsupportedPrimitive {
            operation: "float operation",
        }),
    }
}
