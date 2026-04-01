//! Binary operator implementations for the evaluator.
//!
//! Provides direct enum-based dispatch for binary operations. The type set
//! is fixed (not user-extensible), so pattern matching is preferred over
//! trait objects for better performance and exhaustiveness checking.
//!
//! Registry bridge: `value_to_type_tag()` and `op_strategy_from_op()`
//! connect eval dispatch to the registry's `OpDefs`. Enforcement tests
//! verify the evaluator handles all registry-declared operators.

mod duration_size;

use duration_size::{
    eval_duration_binary, eval_duration_int_binary, eval_int_duration_binary, eval_int_size_binary,
    eval_size_binary, eval_size_int_binary,
};
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
    result
        .map(wrap)
        .ok_or_else(|| integer_overflow(op_name).into())
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
        Err(division_by_zero().into())
    } else {
        op().map(wrap)
            .ok_or_else(|| integer_overflow(op_name).into())
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
        Err(modulo_by_zero().into())
    } else {
        op().map(wrap)
            .ok_or_else(|| integer_overflow(op_name).into())
    }
}

// Registry Bridge

/// Convert a `Value` to its registry `TypeTag` for operator dispatch.
///
/// Returns `None` for compound types (List, Map, Option, Result, etc.) that
/// don't have registry-level operator definitions — their operators are
/// handled by dedicated per-type dispatch functions.
#[cfg(test)]
pub(super) fn value_to_type_tag(v: &Value) -> Option<ori_registry::TypeTag> {
    use ori_registry::TypeTag;
    match v {
        Value::Int(_) => Some(TypeTag::Int),
        Value::Float(_) => Some(TypeTag::Float),
        Value::Bool(_) => Some(TypeTag::Bool),
        Value::Str(_) => Some(TypeTag::Str),
        Value::Char(_) => Some(TypeTag::Char),
        Value::Byte(_) => Some(TypeTag::Byte),
        Value::Duration(_) => Some(TypeTag::Duration),
        Value::Size(_) => Some(TypeTag::Size),
        _ => None,
    }
}

/// Extract the `OpStrategy` for a given `BinaryOp` from a type's `OpDefs`.
///
/// Returns `Unsupported` for operators not in the registry (Range, Coalesce, etc.).
#[cfg(test)]
pub(super) fn op_strategy_from_op(
    ops: &ori_registry::OpDefs,
    op: BinaryOp,
) -> ori_registry::OpStrategy {
    match op {
        BinaryOp::Add => ops.add,
        BinaryOp::Sub => ops.sub,
        BinaryOp::Mul => ops.mul,
        BinaryOp::Div => ops.div,
        BinaryOp::Mod => ops.rem,
        BinaryOp::FloorDiv => ops.floor_div,
        BinaryOp::Eq => ops.eq,
        BinaryOp::NotEq => ops.neq,
        BinaryOp::Lt => ops.lt,
        BinaryOp::Gt => ops.gt,
        BinaryOp::LtEq => ops.lt_eq,
        BinaryOp::GtEq => ops.gt_eq,
        BinaryOp::BitAnd => ops.bit_and,
        BinaryOp::BitOr => ops.bit_or,
        BinaryOp::BitXor => ops.bit_xor,
        BinaryOp::Shl => ops.shl,
        BinaryOp::Shr => ops.shr,
        _ => ori_registry::OpStrategy::Unsupported,
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
    tracing::trace!(
        ?op,
        left_type = left.type_name(),
        right_type = right.type_name(),
        "evaluate_binary"
    );
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
        (Value::Set(a), Value::Set(b)) => eval_set_binary(a, b, op),
        (Value::Struct(a), Value::Struct(b)) => eval_struct_binary(a, b, op),
        (Value::Variant { .. }, Value::Variant { .. }) => eval_variant_binary(&left, &right, op),
        _ => Err(binary_type_mismatch(left.type_name(), right.type_name()).into()),
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
            .ok_or_else(|| shift_out_of_range(b.raw()).into()),
        BinaryOp::Shr => a
            .checked_shr(b)
            .map(Value::Int)
            .ok_or_else(|| shift_out_of_range(b.raw()).into()),
        BinaryOp::Range => Ok(Value::Range(RangeValue::exclusive(a.raw(), b.raw()))),
        BinaryOp::RangeInclusive => Ok(Value::Range(RangeValue::inclusive(a.raw(), b.raw()))),
        _ => Err(invalid_binary_op_for("integers", op).into()),
    }
}

/// Binary operations on floats.
fn eval_float_binary(a: f64, b: f64, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Add => Ok(Value::Float(a + b)),
        BinaryOp::Sub => Ok(Value::Float(a - b)),
        BinaryOp::Mul => Ok(Value::Float(a * b)),
        BinaryOp::Div => Ok(Value::Float(a / b)),
        BinaryOp::Mod => Ok(Value::Float(a % b)),
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
        _ => Err(invalid_binary_op_for("floats", op).into()),
    }
}

/// Binary operations on booleans.
///
/// Supports equality, ordering (false < true, unsigned comparison), and
/// logical operators (&&, ||). Registry: `eq: BoolLogic`, `lt: UnsignedCmp`.
fn eval_bool_binary(a: bool, b: bool, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        // Ordering: false=0 < true=1 (unsigned comparison).
        BinaryOp::Lt => Ok(Value::Bool(!a && b)),
        BinaryOp::LtEq => Ok(Value::Bool(!a || b)),
        BinaryOp::Gt => Ok(Value::Bool(a && !b)),
        BinaryOp::GtEq => Ok(Value::Bool(a || !b)),
        // Logical operators (short-circuit semantics handled by caller;
        // we only reach here for the already-evaluated case).
        BinaryOp::And => Ok(Value::Bool(a && b)),
        BinaryOp::Or => Ok(Value::Bool(a || b)),
        _ => Err(invalid_binary_op_for("booleans", op).into()),
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
        _ => Err(invalid_binary_op_for("strings", op).into()),
    }
}

/// Binary operations on lists.
fn eval_list_binary(
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
fn eval_set_binary(
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

/// Binary operations on characters.
fn eval_char_binary(a: char, b: char, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(a == b)),
        BinaryOp::NotEq => Ok(Value::Bool(a != b)),
        BinaryOp::Lt => Ok(Value::Bool(a < b)),
        BinaryOp::LtEq => Ok(Value::Bool(a <= b)),
        BinaryOp::Gt => Ok(Value::Bool(a > b)),
        BinaryOp::GtEq => Ok(Value::Bool(a >= b)),
        _ => Err(invalid_binary_op_for("char", op).into()),
    }
}

/// Binary operations on tuples.
fn eval_tuple_binary(a: &Heap<Vec<Value>>, b: &Heap<Vec<Value>>, op: BinaryOp) -> EvalResult {
    match op {
        BinaryOp::Eq => Ok(Value::Bool(**a == **b)),
        BinaryOp::NotEq => Ok(Value::Bool(**a != **b)),
        _ => Err(invalid_binary_op_for("tuples", op).into()),
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
            _ => Err(invalid_binary_op_for("Option", op).into()),
        },
        (Value::None, Value::None) => match op {
            BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::GtEq => Ok(Value::Bool(true)),
            BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::Gt => Ok(Value::Bool(false)),
            _ => Err(invalid_binary_op_for("Option", op).into()),
        },
        (Value::None, Value::Some(_)) => match op {
            // None < Some(_) - None is always less than Some
            BinaryOp::Eq | BinaryOp::Gt | BinaryOp::GtEq => Ok(Value::Bool(false)),
            BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::LtEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Option", op).into()),
        },
        (Value::Some(_), Value::None) => match op {
            // Some(_) > None - Some is always greater than None
            BinaryOp::Eq | BinaryOp::Lt | BinaryOp::LtEq => Ok(Value::Bool(false)),
            BinaryOp::NotEq | BinaryOp::Gt | BinaryOp::GtEq => Ok(Value::Bool(true)),
            _ => Err(invalid_binary_op_for("Option", op).into()),
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

// Operator Error Factories

/// Shift amount is outside the valid range for 64-bit integers.
#[cold]
fn shift_out_of_range(amount: i64) -> EvalError {
    EvalError::new(format!("shift amount {amount} out of range (0-63)"))
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
        _ => Err(invalid_binary_op_for("struct", op).into()),
    }
}

/// Binary operations on sum type variants.
///
/// Variants are equal when they share the same type, variant name, and payloads.
fn eval_variant_binary(a: &Value, b: &Value, op: BinaryOp) -> EvalResult {
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

#[cfg(test)]
mod tests;
