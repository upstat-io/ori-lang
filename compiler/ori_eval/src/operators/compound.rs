//! Binary operator dispatch for compound types.
//!
//! Handles list, set, tuple, option, result, struct, and variant operations.
//! These types are not registry-driven — their operators are dispatched
//! directly without consulting `OpDefs`.

use ori_ir::BinaryOp;
use ori_patterns::{invalid_binary_op_for, EvalResult, Heap, Value};

use super::evaluate_binary;

/// Binary operations on lists.
pub(super) fn eval_list_binary(
    a: &ori_patterns::ListData,
    b: &ori_patterns::ListData,
    op: BinaryOp,
) -> EvalResult {
    match op {
        BinaryOp::Add => {
            let mut result = a[..].to_vec();
            result.extend_from_slice(b);
            Ok(Value::list(result))
        }
        BinaryOp::Eq => Ok(Value::Bool(a[..] == b[..])),
        BinaryOp::NotEq => Ok(Value::Bool(a[..] != b[..])),
        _ => Err(invalid_binary_op_for("lists", op).into()),
    }
}

/// Binary operations on sets.
pub(super) fn eval_set_binary(
    a: &Heap<std::collections::BTreeMap<String, Value>>,
    b: &Heap<std::collections::BTreeMap<String, Value>>,
    op: BinaryOp,
) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        _ => Err(invalid_binary_op_for("sets", op).into()),
    }
}

/// Binary operations on tuples.
pub(super) fn eval_tuple_binary(
    a: &Heap<Vec<Value>>,
    b: &Heap<Vec<Value>>,
    op: BinaryOp,
) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        _ => Err(invalid_binary_op_for("tuples", op).into()),
    }
}

/// Binary operations on Option values.
///
/// Per spec: `None < Some` — None is always less than any Some value.
/// For `Some(a)` vs `Some(b)`, recursively compare inner values.
pub(super) fn eval_option_binary(left: &Value, right: &Value, op: BinaryOp) -> EvalResult {
    match (left, right) {
        (Value::Some(a), Value::Some(b)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
            BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                evaluate_binary((**a).clone(), (**b).clone(), op)
            }
            _ => Err(invalid_binary_op_for("Option", op).into()),
        },
        (Value::None, Value::None) => match op {
            BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::GtEq => Ok(Value::Bool(true)),
            BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::Gt => Ok(Value::Bool(false)),
            _ => Err(invalid_binary_op_for("Option", op).into()),
        },
        (Value::None, Value::Some(_)) => match op {
            BinaryOp::Eq | BinaryOp::Gt | BinaryOp::GtEq => Ok(Value::Bool(false)),
            BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::LtEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Option", op).into()),
        },
        (Value::Some(_), Value::None) => match op {
            BinaryOp::Eq | BinaryOp::Lt | BinaryOp::LtEq => Ok(Value::Bool(false)),
            BinaryOp::NotEq | BinaryOp::Gt | BinaryOp::GtEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Option", op).into()),
        },
        _ => unreachable!(),
    }
}

/// Binary operations on Result values.
pub(super) fn eval_result_binary(left: &Value, right: &Value, op: BinaryOp) -> EvalResult {
    match (left, right) {
        (Value::Ok(a), Value::Ok(b)) | (Value::Err(a), Value::Err(b)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(*a == *b)),
            BinaryOp::NotEq => Ok(Value::Bool(*a != *b)),
            _ => Err(invalid_binary_op_for("Result", op).into()),
        },
        (Value::Ok(_), Value::Err(_)) | (Value::Err(_), Value::Ok(_)) => match op {
            BinaryOp::Eq => Ok(Value::Bool(false)),
            BinaryOp::NotEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Result", op).into()),
        },
        _ => unreachable!(),
    }
}

/// Binary operations on struct values.
///
/// Structs support equality comparison. The comparison is structural:
/// both structs must have the same type and all fields must be equal.
pub(super) fn eval_struct_binary(
    a: &ori_patterns::StructValue,
    b: &ori_patterns::StructValue,
    op: BinaryOp,
) -> EvalResult {
    match op {
        BinaryOp::Eq => {
            if a.type_name != b.type_name {
                return Ok(Value::Bool(false));
            }
            Ok(Value::Bool(a.fields == b.fields))
        }
        BinaryOp::NotEq => {
            if a.type_name != b.type_name {
                return Ok(Value::Bool(true));
            }
            Ok(Value::Bool(a.fields != b.fields))
        }
        _ => Err(invalid_binary_op_for("struct", op).into()),
    }
}

/// Binary operations on sum type variants.
///
/// Variants are equal when they share the same type, variant name, and payloads.
pub(super) fn eval_variant_binary(a: &Value, b: &Value, op: BinaryOp) -> EvalResult {
    let (
        Value::Variant {
            type_name: t1,
            variant_name: v1,
            fields: f1,
        },
        Value::Variant {
            type_name: t2,
            variant_name: v2,
            fields: f2,
        },
    ) = (a, b)
    else {
        unreachable!("eval_variant_binary called with non-variant values")
    };

    let equal = t1 == t2 && v1 == v2 && f1 == f2;
    match op {
        BinaryOp::Eq => Ok(Value::Bool(equal)),
        BinaryOp::NotEq => Ok(Value::Bool(!equal)),
        _ => Err(invalid_binary_op_for("variant", op).into()),
    }
}
