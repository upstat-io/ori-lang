//! Ordering type method handler.
//!
//! Provides type inference for methods on the `Ordering` type:
//! - `is_less()`, `is_equal()`, `is_greater()` → `bool`
//! - `is_less_or_equal()`, `is_greater_or_equal()` → `bool`
//! - `reverse()` → `Ordering`

use ori_diagnostic::ErrorCode;
use ori_ir::StringInterner;

use super::{MethodTypeError, MethodTypeResult};
use ori_types::Type;

/// Ordering predicate methods (returning bool).
const ORDERING_BOOL_METHODS: &[&str] = &[
    "is_less",
    "is_equal",
    "is_greater",
    "is_less_or_equal",
    "is_greater_or_equal",
];

/// Check if a type is the Ordering type.
pub fn is_ordering_type(ty: &Type, interner: &StringInterner) -> bool {
    if let Type::Named(name) = ty {
        interner.lookup(*name) == "Ordering"
    } else {
        false
    }
}

/// Type check a method call on an Ordering value.
pub fn check_ordering_method(method: &str, interner: &StringInterner) -> MethodTypeResult {
    // Predicate methods return bool
    if ORDERING_BOOL_METHODS.contains(&method) {
        return MethodTypeResult::Ok(Type::Bool);
    }

    // reverse() returns Ordering
    if method == "reverse" {
        let ordering = interner.intern("Ordering");
        return MethodTypeResult::Ok(Type::Named(ordering));
    }

    // clone() returns Ordering (Clone trait)
    if method == "clone" {
        let ordering = interner.intern("Ordering");
        return MethodTypeResult::Ok(Type::Named(ordering));
    }

    // hash() returns int (Hashable trait)
    if method == "hash" {
        return MethodTypeResult::Ok(Type::Int);
    }

    // to_str() returns str (Printable trait)
    if method == "to_str" {
        return MethodTypeResult::Ok(Type::Str);
    }

    // debug() returns str (Debug trait)
    if method == "debug" {
        return MethodTypeResult::Ok(Type::Str);
    }

    // equals() returns bool (Eq trait)
    if method == "equals" {
        return MethodTypeResult::Ok(Type::Bool);
    }

    // compare() returns Ordering (Comparable trait - Ordering is comparable with itself)
    if method == "compare" {
        let ordering = interner.intern("Ordering");
        return MethodTypeResult::Ok(Type::Named(ordering));
    }

    MethodTypeResult::Err(MethodTypeError::new(
        format!("unknown method `{method}` for type `Ordering`"),
        ErrorCode::E2002,
    ))
}
