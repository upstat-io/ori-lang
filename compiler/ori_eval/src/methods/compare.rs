//! Value comparison utilities.

use std::cmp::Ordering;

use ori_ir::StringInterner;
use ori_patterns::{EvalError, OrderingValue, Value};

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
        // Tuple comparison: lexicographic
        (Value::Tuple(a_elems), Value::Tuple(b_elems)) => compare_lists(a_elems, b_elems, interner),
        // Ordering comparison
        (Value::Ordering(a), Value::Ordering(b)) => Ok(a.to_tag().cmp(&b.to_tag())),
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
pub fn ordering_to_value(ord: Ordering) -> Value {
    Value::ordering_from_cmp(ord)
}

/// Check two values for deep equality.
///
/// Recursive structural equality for compound types. Handles all types
/// that implement the Eq trait. For primitives, delegates to Rust `==`.
/// For containers, recurses element-wise.
#[expect(
    clippy::only_used_in_recursion,
    reason = "interner needed for future struct/newtype deep equality via method dispatch"
)]
pub fn equals_values(a: &Value, b: &Value, interner: &StringInterner) -> Result<bool, EvalError> {
    match (a, b) {
        (Value::Int(a), Value::Int(b)) => Ok(a == b),
        #[expect(
            clippy::float_cmp,
            reason = "Exact float equality is intentional for Eq trait"
        )]
        (Value::Float(a), Value::Float(b)) => Ok(*a == *b),
        (Value::Bool(a), Value::Bool(b)) => Ok(a == b),
        (Value::Str(a), Value::Str(b)) => Ok(**a == **b),
        (Value::Char(a), Value::Char(b)) => Ok(a == b),
        (Value::Byte(a), Value::Byte(b)) => Ok(a == b),
        (Value::Duration(a), Value::Duration(b)) => Ok(a == b),
        (Value::Size(a), Value::Size(b)) => Ok(a == b),
        (Value::Ordering(a), Value::Ordering(b)) => Ok(a == b),
        // Option
        (Value::None, Value::None) => Ok(true),
        (Value::Some(a), Value::Some(b)) => equals_values(a, b, interner),
        // Mismatched Option/Result tags
        (Value::None, Value::Some(_))
        | (Value::Some(_), Value::None)
        | (Value::Ok(_), Value::Err(_))
        | (Value::Err(_), Value::Ok(_)) => Ok(false),
        // Result — matching tags
        (Value::Ok(a), Value::Ok(b)) | (Value::Err(a), Value::Err(b)) => {
            equals_values(a, b, interner)
        }
        // List/Tuple — element-wise equality
        (Value::List(a), Value::List(b)) | (Value::Tuple(a), Value::Tuple(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (ai, bi) in a.iter().zip(b.iter()) {
                if !equals_values(ai, bi, interner)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        // Map/Set — same keys with same values (order-independent, BTreeMap-based)
        (Value::Map(a), Value::Map(b)) | (Value::Set(a), Value::Set(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (key, a_val) in a.iter() {
                match b.get(key.as_str()) {
                    Some(b_val) => {
                        if !equals_values(a_val, b_val, interner)? {
                            return Ok(false);
                        }
                    }
                    None => return Ok(false),
                }
            }
            Ok(true)
        }
        _ => Err(EvalError::new(format!(
            "cannot compare {} with {} for equality",
            a.type_name(),
            b.type_name()
        ))),
    }
}

/// Combine two hash values using the Boost hash combine algorithm.
///
/// Uses the golden ratio constant `0x9e3779b9` to mix bits, providing good
/// distribution across the hash space. This is the same algorithm exposed
/// to Ori users as the `hash_combine` prelude function.
pub fn hash_combine(seed: i64, value: i64) -> i64 {
    seed ^ (value
        .wrapping_add(0x9e37_79b9_i64)
        .wrapping_add(seed << 6)
        .wrapping_add(seed >> 2))
}

/// Compute the hash of a value recursively.
///
/// Handles all hashable types. For primitives, uses identity (int) or
/// `DefaultHasher` (str). For compound types, combines element hashes
/// with `hash_combine`. Float normalization ensures `-0.0` and `+0.0`
/// produce the same hash, and all NaN representations hash identically.
#[expect(
    clippy::only_used_in_recursion,
    reason = "interner needed for future struct/newtype deep hashing via method dispatch"
)]
pub fn hash_value(v: &Value, interner: &StringInterner) -> Result<i64, EvalError> {
    match v {
        Value::Int(n) => Ok(n.raw()),
        Value::Float(f) => Ok(hash_float(*f)),
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Str(s) => {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            s.hash(&mut hasher);
            Ok(hasher.finish().cast_signed())
        }
        Value::Char(c) => Ok(i64::from(*c as u32)),
        Value::Byte(b) => Ok(i64::from(*b)),
        Value::Duration(d) => Ok(*d),
        Value::Size(s) => Ok((*s).cast_signed()),
        Value::Ordering(o) => Ok(i64::from(o.to_tag())),
        // Option: None → 0, Some(x) → hash_combine(1, hash(x))
        Value::None => Ok(0),
        Value::Some(inner) => {
            let inner_hash = hash_value(inner, interner)?;
            Ok(hash_combine(1, inner_hash))
        }
        // Result: Ok(x) → hash_combine(2, hash(x)), Err(x) → hash_combine(3, hash(x))
        Value::Ok(inner) => {
            let inner_hash = hash_value(inner, interner)?;
            Ok(hash_combine(2, inner_hash))
        }
        Value::Err(inner) => {
            let inner_hash = hash_value(inner, interner)?;
            Ok(hash_combine(3, inner_hash))
        }
        // List/Tuple: fold with hash_combine
        Value::List(items) | Value::Tuple(items) => {
            let mut h = 0_i64;
            for item in items.iter() {
                h = hash_combine(h, hash_value(item, interner)?);
            }
            Ok(h)
        }
        // Map/Set: XOR all entry hashes (order-independent)
        Value::Map(map) => {
            let mut h = 0_i64;
            for (key, val) in map.iter() {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                key.hash(&mut hasher);
                let key_hash = hasher.finish().cast_signed();
                let val_hash = hash_value(val, interner)?;
                h ^= hash_combine(key_hash, val_hash);
            }
            Ok(h)
        }
        Value::Set(items) => {
            let mut h = 0_i64;
            for val in items.values() {
                h ^= hash_value(val, interner)?;
            }
            Ok(h)
        }
        _ => Err(EvalError::new(format!("cannot hash {}", v.type_name()))),
    }
}

/// Hash a float with normalization.
///
/// Ensures: `-0.0` and `+0.0` produce the same hash (both equal via `==`),
/// and all NaN representations hash identically (since `NaN != NaN`,
/// Hashable trait doesn't require NaN consistency, but we normalize anyway).
pub(super) fn hash_float(f: f64) -> i64 {
    let normalized = if f == 0.0 {
        0.0_f64
    } else if f.is_nan() {
        f64::NAN
    } else {
        f
    };
    normalized.to_bits().cast_signed()
}

/// Extract `OrderingValue` from `Value::Ordering`.
pub fn extract_ordering(value: &Value) -> Option<OrderingValue> {
    match value {
        Value::Ordering(ord) => Some(*ord),
        _ => None,
    }
}
