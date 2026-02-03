//! Value comparison utilities.

use ori_ir::StringInterner;
use ori_patterns::{EvalError, OrderingValue, Value};
use std::cmp::Ordering;

/// Compare two Option values.
///
/// Per spec: None < Some(_). When both are Some, compare inner values.
pub fn compare_option_values(
    a: &Value,
    b: &Value,
    interner: &StringInterner,
) -> Result<Ordering, EvalError> {
    match (a, b) {
        (Value::None, Value::None) => Ok(Ordering::Equal),
        (Value::None, Value::Some(_)) => Ok(Ordering::Less),
        (Value::Some(_), Value::None) => Ok(Ordering::Greater),
        (Value::Some(a_inner), Value::Some(b_inner)) => compare_values(a_inner, b_inner, interner),
        _ => Err(EvalError::new("compare requires Option values")),
    }
}

/// Compare two values of the same type.
///
/// Used for comparing inner values of Option and other compound types.
pub fn compare_values(
    a: &Value,
    b: &Value,
    interner: &StringInterner,
) -> Result<Ordering, EvalError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(a.cmp(b)),
        (Value::Float(a), Value::Float(b)) => Ok(a.total_cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Ok(a.cmp(b)),
        (Value::Str(a), Value::Str(b)) => Ok((**a).cmp(&**b)),
        (Value::Char(a), Value::Char(b)) => Ok(a.cmp(b)),
        (Value::Byte(a), Value::Byte(b)) => Ok(a.cmp(b)),
        (Value::Duration(a), Value::Duration(b)) => Ok(a.cmp(b)),
        (Value::Size(a), Value::Size(b)) => Ok(a.cmp(b)),
        (Value::None, Value::None) => Ok(Ordering::Equal),
        (Value::None, Value::Some(_)) | (Value::Ok(_), Value::Err(_)) => Ok(Ordering::Less),
        (Value::Some(_), Value::None) | (Value::Err(_), Value::Ok(_)) => Ok(Ordering::Greater),
        (Value::Some(a_inner), Value::Some(b_inner))
        | (Value::Ok(a_inner), Value::Ok(b_inner))
        | (Value::Err(a_inner), Value::Err(b_inner)) => compare_values(a_inner, b_inner, interner),
        // List comparison: lexicographic
        (Value::List(a_items), Value::List(b_items)) => compare_lists(a_items, b_items, interner),
        _ => Err(EvalError::new(format!(
            "cannot compare {} with {}",
            a.type_name(),
            b.type_name()
        ))),
    }
}

/// Compare two lists lexicographically.
///
/// Compares element by element. First difference determines the result.
/// If one is a prefix of the other, the shorter list is less.
pub fn compare_lists(
    a: &[Value],
    b: &[Value],
    interner: &StringInterner,
) -> Result<Ordering, EvalError> {
    for (a_item, b_item) in a.iter().zip(b.iter()) {
        let ord = compare_values(a_item, b_item, interner)?;
        if ord != Ordering::Equal {
            return Ok(ord);
        }
    }
    // All compared elements are equal, compare lengths
    Ok(a.len().cmp(&b.len()))
}

/// Compare two Result values.
///
/// Per spec: Ok(_) < Err(_). When both are same variant, compare inner values.
pub fn compare_result_values(
    a: &Value,
    b: &Value,
    interner: &StringInterner,
) -> Result<Ordering, EvalError> {
    match (a, b) {
        (Value::Ok(a_inner), Value::Ok(b_inner)) | (Value::Err(a_inner), Value::Err(b_inner)) => {
            compare_values(a_inner, b_inner, interner)
        }
        (Value::Ok(_), Value::Err(_)) => Ok(Ordering::Less),
        (Value::Err(_), Value::Ok(_)) => Ok(Ordering::Greater),
        _ => Err(EvalError::new("compare requires Result values")),
    }
}

/// Convert Rust Ordering to Ori Ordering value.
///
/// Creates a first-class `Value::Ordering` value.
pub fn ordering_to_value(ord: Ordering, _interner: &StringInterner) -> Value {
    Value::ordering_from_cmp(ord)
}

/// Extract `OrderingValue` from `Value::Ordering`.
pub fn extract_ordering(value: &Value) -> Option<OrderingValue> {
    match value {
        Value::Ordering(ord) => Some(*ord),
        _ => None,
    }
}
