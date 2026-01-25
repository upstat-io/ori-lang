//! Centralized error constructors for the evaluator.
//!
//! This module provides a single import point for all evaluation error constructors.
//! Centralizing errors here makes future internationalization straightforward -
//! error messages can be replaced with translation keys in one location.
//!
//! # Usage
//!
//! ```ignore
//! use sigil_eval::errors::{undefined_variable, division_by_zero};
//! ```

// Re-export EvalError and EvalResult types
pub use sigil_patterns::{EvalError, EvalResult};

// =============================================================================
// Binary Operation Errors
// =============================================================================

pub use sigil_patterns::{
    binary_type_mismatch,
    division_by_zero,
    invalid_binary_op,
    modulo_by_zero,
};

// =============================================================================
// Method Call Errors
// =============================================================================

pub use sigil_patterns::{
    no_such_method,
    wrong_arg_count,
    wrong_arg_type,
};

// =============================================================================
// Variable and Function Errors
// =============================================================================

pub use sigil_patterns::{
    not_callable,
    undefined_config,
    undefined_function,
    undefined_variable,
    wrong_function_args,
};

// =============================================================================
// Index and Field Access Errors
// =============================================================================

pub use sigil_patterns::{
    cannot_access_field,
    cannot_get_length,
    cannot_index,
    index_out_of_bounds,
    invalid_tuple_field,
    key_not_found,
    no_field_on_struct,
    tuple_index_out_of_bounds,
};

// =============================================================================
// Type Conversion and Validation Errors
// =============================================================================

pub use sigil_patterns::{
    map_keys_must_be_strings,
    range_bound_not_int,
    unbounded_range_end,
};

// =============================================================================
// Control Flow Errors
// =============================================================================

pub use sigil_patterns::{
    cannot_assign_immutable,
    for_requires_iterable,
    invalid_assignment_target,
    non_exhaustive_match,
};

// =============================================================================
// Pattern Binding Errors
// =============================================================================

pub use sigil_patterns::{
    expected_list,
    expected_struct,
    expected_tuple,
    list_pattern_too_long,
    missing_struct_field,
    tuple_pattern_mismatch,
};

// =============================================================================
// Miscellaneous Errors
// =============================================================================

pub use sigil_patterns::{
    await_not_supported,
    hash_outside_index,
    invalid_literal_pattern,
    parse_error,
    self_outside_method,
};
