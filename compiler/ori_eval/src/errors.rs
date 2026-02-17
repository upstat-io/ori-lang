//! Centralized error constructors for the evaluator.
//!
//! This module provides a single import point for all evaluation error constructors.
//! Shared errors (type mismatches, undefined variables, etc.) are re-exported from
//! `ori_patterns`. Eval-specific errors (assignment semantics, not-implemented features,
//! interpreter-only constructs) are defined locally here.
//!
//! # Usage
//!
//!     use ori_eval::errors::{undefined_variable, division_by_zero};

// Re-export EvalError and EvalResult types
pub use ori_patterns::{EvalError, EvalErrorKind, EvalResult};

// Binary Operation Errors

pub use ori_patterns::{binary_type_mismatch, division_by_zero, modulo_by_zero};

// Method Call Errors

pub use ori_patterns::{no_such_method, wrong_arg_count, wrong_arg_type};

// Variable and Function Errors

pub use ori_patterns::{
    not_callable, undefined_const, undefined_function, undefined_variable, wrong_function_args,
};

// Index and Field Access Errors

pub use ori_patterns::{
    cannot_access_field, cannot_get_length, cannot_index, index_out_of_bounds, invalid_tuple_field,
    no_field_on_struct, no_member_in_module, tuple_index_out_of_bounds,
};

// Type Conversion and Validation Errors

pub use ori_patterns::{
    map_key_not_hashable, range_bound_not_int, unbounded_range_eager, unbounded_range_length,
};

// Control Flow Errors (shared)

pub use ori_patterns::{for_requires_iterable, invalid_assignment_target, non_exhaustive_match};

// Pattern Binding Errors

pub use ori_patterns::{
    expected_list, expected_struct, expected_tuple, list_pattern_too_long, missing_struct_field,
    tuple_pattern_mismatch,
};

// Miscellaneous Errors (shared)

pub use ori_patterns::{await_not_supported, hash_outside_index, parse_error, self_outside_method};

// Collection Method Errors

pub use ori_patterns::{
    all_requires_list, any_requires_list, collect_requires_range, filter_entries_requires_map,
    filter_requires_collection, find_requires_list, fold_requires_collection, join_requires_list,
    map_entries_requires_map, map_requires_collection,
};

// Index Context Errors

pub use ori_patterns::collection_too_large;

// Eval-specific errors
//
// These errors are specific to interpreter evaluation semantics and do not
// belong in `ori_patterns` (a shared pattern-matching library). New eval-only
// errors should be added here, not upstream.

/// Cannot assign to immutable variable.
#[cold]
pub fn cannot_assign_immutable(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::ImmutableBinding {
        name: name.to_string(),
    })
}

/// Index assignment (`list[i] = x`) is not supported.
#[cold]
pub fn index_assignment_not_supported() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "index assignment (list[i] = x) is not supported".to_string(),
        suggestion: "use functional update patterns instead".to_string(),
    })
}

/// Field assignment (`obj.field = x`) not yet implemented.
#[cold]
pub fn field_assignment_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "field assignment (obj.field = x) is not yet implemented".to_string(),
        suggestion: "use spread syntax: { ...obj, field: x }".to_string(),
    })
}

/// `default()` requires type context.
#[cold]
pub fn default_requires_type_context() -> EvalError {
    EvalError::new("default() requires type context; use explicit construction instead")
}

/// `map_entries()` not yet implemented.
#[cold]
pub fn map_entries_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "map_entries() is not yet implemented".to_string(),
        suggestion: "use map() with entry destructuring: map.entries().map((k, v) -> ...)"
            .to_string(),
    })
}

/// `filter_entries()` not yet implemented.
#[cold]
pub fn filter_entries_not_implemented() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotImplemented {
        feature: "filter_entries() is not yet implemented".to_string(),
        suggestion: "use filter() with entry destructuring: map.entries().filter((k, v) -> ...)"
            .to_string(),
    })
}

/// Const-eval budget exceeded.
#[cold]
pub fn budget_exceeded(calls: usize, budget: u32) -> EvalError {
    EvalError::new(format!(
        "const-eval budget exceeded: {calls} calls (limit: {budget})"
    ))
}
