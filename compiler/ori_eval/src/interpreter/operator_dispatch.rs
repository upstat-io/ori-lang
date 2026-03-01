//! Operator dispatch helpers — primitivity checks and op→method name mapping.
//!
//! Extracted from `interpreter/mod.rs` to keep the main module focused on
//! interpreter structure and core dispatch.

use ori_ir::{BinaryOp, Name, UnaryOp};
use ori_patterns::Value;

use super::OpNames;

/// Check if a value is a primitive type that uses built-in operator evaluation.
///
/// Primitive types (int, float, bool, str, char, byte, Duration, Size) use direct
/// evaluation via `evaluate_binary`. User-defined types dispatch through operator
/// trait methods (`Add::add`, `Sub::subtract`, `Mul::multiply`, etc.).
pub(super) fn is_primitive_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Int(_)
            | Value::Float(_)
            | Value::Bool(_)
            | Value::Str(_)
            | Value::Char(_)
            | Value::Byte(_)
            | Value::Duration(_)
            | Value::Size(_)
            | Value::List(_)
            | Value::Tuple(_)
            | Value::Map(_)
            | Value::Some(_)
            | Value::None
            | Value::Ok(_)
            | Value::Err(_)
            | Value::Range(_)
    )
}

/// Check if a value is a built-in indexable type (list, map, str, tuple).
///
/// These types use the fast-path direct indexing with `#` hash-length support.
/// User-defined types dispatch through `Index` trait methods instead.
pub(super) fn is_builtin_indexable(value: &Value) -> bool {
    matches!(
        value,
        Value::List(_) | Value::Map(_) | Value::Str(_) | Value::Tuple(_)
    )
}

/// Map a binary operator to its pre-interned trait method name.
///
/// Returns `Some(Name)` for operators that have trait implementations,
/// or `None` for comparison, logical, range, and null-coalescing operators
/// which use direct evaluation.
pub(super) fn binary_op_to_method(op: BinaryOp, names: OpNames) -> Option<Name> {
    match op {
        // Arithmetic operators
        BinaryOp::Add => Some(names.add),
        BinaryOp::Sub => Some(names.subtract),
        BinaryOp::Mul => Some(names.multiply),
        // Note: "divide" not "div" because `div` is a keyword (floor division operator)
        BinaryOp::Div => Some(names.divide),
        BinaryOp::FloorDiv => Some(names.floor_divide),
        BinaryOp::Mod => Some(names.remainder),
        // Bitwise operators
        BinaryOp::BitAnd => Some(names.bit_and),
        BinaryOp::BitOr => Some(names.bit_or),
        BinaryOp::BitXor => Some(names.bit_xor),
        BinaryOp::Shl => Some(names.shift_left),
        BinaryOp::Shr => Some(names.shift_right),
        BinaryOp::MatMul => Some(names.mat_mul),
        // Comparison, logical, range, and null-coalescing operators
        // use direct evaluation (no trait method)
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::Range
        | BinaryOp::RangeInclusive
        | BinaryOp::Coalesce => None,
    }
}

/// Map a unary operator to its pre-interned trait method name.
///
/// Returns `Some(Name)` for operators that have trait implementations,
/// or `None` for the Try operator which doesn't have a trait.
pub(super) fn unary_op_to_method(op: UnaryOp, names: OpNames) -> Option<Name> {
    match op {
        UnaryOp::Neg => Some(names.negate),
        UnaryOp::Not => Some(names.not),
        UnaryOp::BitNot => Some(names.bit_not),
        UnaryOp::Try => None, // Try operator doesn't have a trait
    }
}
