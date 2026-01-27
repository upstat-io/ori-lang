//! Centralized error constructors for the evaluator.
//!
//! This module provides a single import point for all evaluation error constructors.
//! Centralizing errors here makes future internationalization straightforward -
//! error messages can be replaced with translation keys in one location.
//!
//! # Usage
//!
//! ```ignore
//! use ori_eval::errors::{undefined_variable, division_by_zero};
//! ```

// Re-export EvalError and EvalResult types
pub use ori_patterns::{EvalError, EvalResult};

// Binary Operation Errors

pub use ori_patterns::{binary_type_mismatch, division_by_zero, invalid_binary_op, modulo_by_zero};

// Method Call Errors

pub use ori_patterns::{no_such_method, wrong_arg_count, wrong_arg_type};

// Variable and Function Errors

pub use ori_patterns::{
    not_callable, undefined_config, undefined_function, undefined_variable, wrong_function_args,
};

// Index and Field Access Errors

pub use ori_patterns::{
    cannot_access_field, cannot_get_length, cannot_index, index_out_of_bounds, invalid_tuple_field,
    key_not_found, no_field_on_struct, tuple_index_out_of_bounds,
};

// Type Conversion and Validation Errors

pub use ori_patterns::{map_keys_must_be_strings, range_bound_not_int, unbounded_range_end};

// Control Flow Errors

pub use ori_patterns::{
    cannot_assign_immutable, for_requires_iterable, invalid_assignment_target, non_exhaustive_match,
};

// Pattern Binding Errors

pub use ori_patterns::{
    expected_list, expected_struct, expected_tuple, list_pattern_too_long, missing_struct_field,
    tuple_pattern_mismatch,
};

// Miscellaneous Errors

pub use ori_patterns::{
    await_not_supported, hash_outside_index, invalid_literal_pattern, parse_error,
    self_outside_method,
};

// Collection Method Errors

pub use ori_patterns::{
    all_requires_list, any_requires_list, collect_requires_range, filter_entries_requires_map,
    filter_requires_collection, find_requires_list, fold_requires_collection,
    map_entries_requires_map, map_requires_collection,
};

// Not Implemented Errors

pub use ori_patterns::{
    default_requires_type_context, field_assignment_not_implemented,
    filter_entries_not_implemented, index_assignment_not_implemented, map_entries_not_implemented,
};

// Index Context Errors

pub use ori_patterns::{
    collection_too_large, non_integer_in_index, operator_not_supported_in_index,
};

// Pattern Errors

pub use ori_patterns::{for_pattern_requires_list, unknown_pattern};
