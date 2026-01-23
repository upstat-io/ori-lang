//! Centralized error message constructors for the evaluator.
//!
//! This module provides consistent error messages across the evaluator,
//! eliminating duplicate string literals and ensuring uniform error formatting.

use super::evaluator::EvalError;

// =============================================================================
// Binary Operation Errors
// =============================================================================

/// Invalid operator for a specific type.
pub fn invalid_binary_op(type_name: &str) -> EvalError {
    EvalError::new(format!("invalid operator for {}", type_name))
}

/// Type mismatch in binary operation.
pub fn binary_type_mismatch(left: &str, right: &str) -> EvalError {
    EvalError::new(format!(
        "type mismatch in binary operation: {} and {}",
        left, right
    ))
}

/// Division by zero error.
pub fn division_by_zero() -> EvalError {
    EvalError::new("division by zero")
}

/// Modulo by zero error.
pub fn modulo_by_zero() -> EvalError {
    EvalError::new("modulo by zero")
}

// =============================================================================
// Method Call Errors
// =============================================================================

/// No such method on a type.
pub fn no_such_method(method: &str, type_name: &str) -> EvalError {
    EvalError::new(format!("no method '{}' on type {}", method, type_name))
}

/// Wrong argument count for a method.
pub fn wrong_arg_count(method: &str, expected: usize, got: usize) -> EvalError {
    EvalError::new(format!(
        "{} expects {} argument(s), got {}",
        method, expected, got
    ))
}

/// Wrong argument type for a method.
pub fn wrong_arg_type(method: &str, expected: &str) -> EvalError {
    EvalError::new(format!("{} expects a {} argument", method, expected))
}

// =============================================================================
// Variable and Function Errors
// =============================================================================

/// Undefined variable.
pub fn undefined_variable(name: &str) -> EvalError {
    EvalError::new(format!("undefined variable: {}", name))
}

/// Undefined function.
pub fn undefined_function(name: &str) -> EvalError {
    EvalError::new(format!("undefined function: @{}", name))
}

/// Undefined config.
pub fn undefined_config(name: &str) -> EvalError {
    EvalError::new(format!("undefined config: ${}", name))
}

/// Value is not callable.
pub fn not_callable(type_name: &str) -> EvalError {
    EvalError::new(format!("{} is not callable", type_name))
}

/// Wrong number of arguments in function call.
pub fn wrong_function_args(expected: usize, got: usize) -> EvalError {
    EvalError::new(format!("expected {} arguments, got {}", expected, got))
}

// =============================================================================
// Index and Field Access Errors
// =============================================================================

/// Index out of bounds.
pub fn index_out_of_bounds(index: i64) -> EvalError {
    EvalError::new(format!("index {} out of bounds", index))
}

/// Key not found in map.
pub fn key_not_found(key: &str) -> EvalError {
    EvalError::new(format!("key not found: {}", key))
}

/// Cannot index type with another type.
pub fn cannot_index(receiver: &str, index: &str) -> EvalError {
    EvalError::new(format!("cannot index {} with {}", receiver, index))
}

/// Cannot get length of type.
pub fn cannot_get_length(type_name: &str) -> EvalError {
    EvalError::new(format!("cannot get length of {}", type_name))
}

/// No field on struct.
pub fn no_field_on_struct(field: &str) -> EvalError {
    EvalError::new(format!("no field {} on struct", field))
}

/// Invalid tuple field.
pub fn invalid_tuple_field(field: &str) -> EvalError {
    EvalError::new(format!("invalid tuple field: {}", field))
}

/// Tuple index out of bounds.
pub fn tuple_index_out_of_bounds(index: usize) -> EvalError {
    EvalError::new(format!("tuple index {} out of bounds", index))
}

/// Cannot access field on type.
pub fn cannot_access_field(type_name: &str) -> EvalError {
    EvalError::new(format!("cannot access field on {}", type_name))
}

// =============================================================================
// Type Conversion and Validation Errors
// =============================================================================

/// Range start/end must be integer.
pub fn range_bound_not_int(bound: &str) -> EvalError {
    EvalError::new(format!("range {} must be an integer", bound))
}

/// Unbounded range end.
pub fn unbounded_range_end() -> EvalError {
    EvalError::new("unbounded range end")
}

/// Map keys must be strings.
pub fn map_keys_must_be_strings() -> EvalError {
    EvalError::new("map keys must be strings")
}

// =============================================================================
// Control Flow Errors
// =============================================================================

/// Non-exhaustive match.
pub fn non_exhaustive_match() -> EvalError {
    EvalError::new("non-exhaustive match")
}

/// Cannot assign to immutable variable.
pub fn cannot_assign_immutable(name: &str) -> EvalError {
    EvalError::new(format!("cannot assign to immutable variable: {}", name))
}

/// Invalid assignment target.
pub fn invalid_assignment_target() -> EvalError {
    EvalError::new("invalid assignment target")
}

/// For loop requires iterable.
pub fn for_requires_iterable() -> EvalError {
    EvalError::new("for requires an iterable")
}

// =============================================================================
// Pattern Binding Errors
// =============================================================================

/// Tuple pattern length mismatch.
pub fn tuple_pattern_mismatch() -> EvalError {
    EvalError::new("tuple pattern length mismatch")
}

/// Expected tuple value.
pub fn expected_tuple() -> EvalError {
    EvalError::new("expected tuple value")
}

/// Expected struct value.
pub fn expected_struct() -> EvalError {
    EvalError::new("expected struct value")
}

/// Expected list value.
pub fn expected_list() -> EvalError {
    EvalError::new("expected list value")
}

/// List pattern too long for value.
pub fn list_pattern_too_long() -> EvalError {
    EvalError::new("list pattern too long for value")
}

/// Missing struct field.
pub fn missing_struct_field() -> EvalError {
    EvalError::new("missing struct field")
}

// =============================================================================
// Miscellaneous Errors
// =============================================================================

/// Self used outside of method context.
pub fn self_outside_method() -> EvalError {
    EvalError::new("'self' used outside of method context")
}

/// Parse error placeholder.
pub fn parse_error() -> EvalError {
    EvalError::new("parse error")
}

/// Hash length used outside index brackets.
pub fn hash_outside_index() -> EvalError {
    EvalError::new("# can only be used inside index brackets")
}

/// Await not supported.
pub fn await_not_supported() -> EvalError {
    EvalError::new("await not supported in interpreter")
}

/// Invalid literal pattern.
pub fn invalid_literal_pattern() -> EvalError {
    EvalError::new("invalid literal pattern")
}
