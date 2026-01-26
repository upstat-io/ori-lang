//! Error types for pattern evaluation.
//!
//! This module provides the core error types used during pattern evaluation
//! and the evaluator's runtime.

use crate::value::Value;

/// Result of evaluation.
pub type EvalResult = Result<Value, EvalError>;

/// Evaluation error.
#[derive(Clone, Debug)]
pub struct EvalError {
    /// Error message.
    pub message: String,
    /// If this error is from `?` propagation, holds the original Err/None value.
    pub propagated_value: Option<Value>,
}

impl EvalError {
    pub fn new(message: impl Into<String>) -> Self {
        EvalError {
            message: message.into(),
            propagated_value: None,
        }
    }

    /// Create an error for propagating an Err or None value.
    pub fn propagate(value: Value, message: impl Into<String>) -> Self {
        EvalError {
            message: message.into(),
            propagated_value: Some(value),
        }
    }
}

// =============================================================================
// Binary Operation Errors
// =============================================================================

/// Invalid operator for a specific type.
#[cold]
pub fn invalid_binary_op(type_name: &str) -> EvalError {
    EvalError::new(format!("invalid operator for {type_name}"))
}

/// Type mismatch in binary operation.
#[cold]
pub fn binary_type_mismatch(left: &str, right: &str) -> EvalError {
    EvalError::new(format!(
        "type mismatch in binary operation: {left} and {right}"
    ))
}

/// Division by zero error.
#[cold]
pub fn division_by_zero() -> EvalError {
    EvalError::new("division by zero")
}

/// Modulo by zero error.
#[cold]
pub fn modulo_by_zero() -> EvalError {
    EvalError::new("modulo by zero")
}

// =============================================================================
// Method Call Errors
// =============================================================================

/// No such method on a type.
#[cold]
pub fn no_such_method(method: &str, type_name: &str) -> EvalError {
    EvalError::new(format!("no method '{method}' on type {type_name}"))
}

/// Wrong argument count for a method.
#[cold]
pub fn wrong_arg_count(method: &str, expected: usize, got: usize) -> EvalError {
    EvalError::new(format!(
        "{method} expects {expected} argument(s), got {got}"
    ))
}

/// Wrong argument type for a method.
#[cold]
pub fn wrong_arg_type(method: &str, expected: &str) -> EvalError {
    EvalError::new(format!("{method} expects a {expected} argument"))
}

// =============================================================================
// Variable and Function Errors
// =============================================================================

/// Undefined variable.
#[cold]
pub fn undefined_variable(name: &str) -> EvalError {
    EvalError::new(format!("undefined variable: {name}"))
}

/// Undefined function.
#[cold]
pub fn undefined_function(name: &str) -> EvalError {
    EvalError::new(format!("undefined function: @{name}"))
}

/// Undefined config.
#[cold]
pub fn undefined_config(name: &str) -> EvalError {
    EvalError::new(format!("undefined config: ${name}"))
}

/// Value is not callable.
#[cold]
pub fn not_callable(type_name: &str) -> EvalError {
    EvalError::new(format!("{type_name} is not callable"))
}

/// Wrong number of arguments in function call.
#[cold]
pub fn wrong_function_args(expected: usize, got: usize) -> EvalError {
    EvalError::new(format!("expected {expected} arguments, got {got}"))
}

// =============================================================================
// Index and Field Access Errors
// =============================================================================

/// Index out of bounds.
#[cold]
pub fn index_out_of_bounds(index: i64) -> EvalError {
    EvalError::new(format!("index {index} out of bounds"))
}

/// Key not found in map.
#[cold]
pub fn key_not_found(key: &str) -> EvalError {
    EvalError::new(format!("key not found: {key}"))
}

/// Cannot index type with another type.
#[cold]
pub fn cannot_index(receiver: &str, index: &str) -> EvalError {
    EvalError::new(format!("cannot index {receiver} with {index}"))
}

/// Cannot get length of type.
#[cold]
pub fn cannot_get_length(type_name: &str) -> EvalError {
    EvalError::new(format!("cannot get length of {type_name}"))
}

/// No field on struct.
#[cold]
pub fn no_field_on_struct(field: &str) -> EvalError {
    EvalError::new(format!("no field {field} on struct"))
}

/// Invalid tuple field.
#[cold]
pub fn invalid_tuple_field(field: &str) -> EvalError {
    EvalError::new(format!("invalid tuple field: {field}"))
}

/// Tuple index out of bounds.
#[cold]
pub fn tuple_index_out_of_bounds(index: usize) -> EvalError {
    EvalError::new(format!("tuple index {index} out of bounds"))
}

/// Cannot access field on type.
#[cold]
pub fn cannot_access_field(type_name: &str) -> EvalError {
    EvalError::new(format!("cannot access field on {type_name}"))
}

// =============================================================================
// Type Conversion and Validation Errors
// =============================================================================

/// Range start/end must be integer.
#[cold]
pub fn range_bound_not_int(bound: &str) -> EvalError {
    EvalError::new(format!("range {bound} must be an integer"))
}

/// Unbounded range end.
#[cold]
pub fn unbounded_range_end() -> EvalError {
    EvalError::new("unbounded range end")
}

/// Map keys must be strings.
#[cold]
pub fn map_keys_must_be_strings() -> EvalError {
    EvalError::new("map keys must be strings")
}

// =============================================================================
// Control Flow Errors
// =============================================================================

/// Non-exhaustive match.
#[cold]
pub fn non_exhaustive_match() -> EvalError {
    EvalError::new("non-exhaustive match")
}

/// Cannot assign to immutable variable.
#[cold]
pub fn cannot_assign_immutable(name: &str) -> EvalError {
    EvalError::new(format!("cannot assign to immutable variable: {name}"))
}

/// Invalid assignment target.
#[cold]
pub fn invalid_assignment_target() -> EvalError {
    EvalError::new("invalid assignment target")
}

/// For loop requires iterable.
#[cold]
pub fn for_requires_iterable() -> EvalError {
    EvalError::new("for requires an iterable")
}

// =============================================================================
// Pattern Binding Errors
// =============================================================================

/// Tuple pattern length mismatch.
#[cold]
pub fn tuple_pattern_mismatch() -> EvalError {
    EvalError::new("tuple pattern length mismatch")
}

/// Expected tuple value.
#[cold]
pub fn expected_tuple() -> EvalError {
    EvalError::new("expected tuple value")
}

/// Expected struct value.
#[cold]
pub fn expected_struct() -> EvalError {
    EvalError::new("expected struct value")
}

/// Expected list value.
#[cold]
pub fn expected_list() -> EvalError {
    EvalError::new("expected list value")
}

/// List pattern too long for value.
#[cold]
pub fn list_pattern_too_long() -> EvalError {
    EvalError::new("list pattern too long for value")
}

/// Missing struct field.
#[cold]
pub fn missing_struct_field() -> EvalError {
    EvalError::new("missing struct field")
}

// =============================================================================
// Miscellaneous Errors
// =============================================================================

/// Self used outside of method context.
#[cold]
pub fn self_outside_method() -> EvalError {
    EvalError::new("'self' used outside of method context")
}

/// Parse error placeholder.
#[cold]
pub fn parse_error() -> EvalError {
    EvalError::new("parse error")
}

/// Hash length used outside index brackets.
#[cold]
pub fn hash_outside_index() -> EvalError {
    EvalError::new("# can only be used inside index brackets")
}

/// Await not supported.
#[cold]
pub fn await_not_supported() -> EvalError {
    EvalError::new("await not supported in interpreter")
}

/// Invalid literal pattern.
#[cold]
pub fn invalid_literal_pattern() -> EvalError {
    EvalError::new("invalid literal pattern")
}

// =============================================================================
// Collection Method Errors
// =============================================================================

/// Map requires a collection (list or range).
#[cold]
pub fn map_requires_collection() -> EvalError {
    EvalError::new("map requires a collection")
}

/// Filter requires a collection (list or range).
#[cold]
pub fn filter_requires_collection() -> EvalError {
    EvalError::new("filter requires a collection")
}

/// Fold requires a collection (list or range).
#[cold]
pub fn fold_requires_collection() -> EvalError {
    EvalError::new("fold requires a collection")
}

/// Find requires a list.
#[cold]
pub fn find_requires_list() -> EvalError {
    EvalError::new("find requires a list")
}

/// Collect requires a range.
#[cold]
pub fn collect_requires_range() -> EvalError {
    EvalError::new("collect requires a range")
}

/// Any requires a list.
#[cold]
pub fn any_requires_list() -> EvalError {
    EvalError::new("any requires a list")
}

/// All requires a list.
#[cold]
pub fn all_requires_list() -> EvalError {
    EvalError::new("all requires a list")
}

/// Map entries requires a map.
#[cold]
pub fn map_entries_requires_map() -> EvalError {
    EvalError::new("map entries requires a map")
}

/// Filter entries requires a map.
#[cold]
pub fn filter_entries_requires_map() -> EvalError {
    EvalError::new("filter entries requires a map")
}

// =============================================================================
// Not Implemented Errors
// =============================================================================

/// Map entries not yet implemented.
#[cold]
pub fn map_entries_not_implemented() -> EvalError {
    EvalError::new("map entries not yet implemented")
}

/// Filter entries not yet implemented.
#[cold]
pub fn filter_entries_not_implemented() -> EvalError {
    EvalError::new("filter entries not yet implemented")
}

/// Index assignment not yet implemented.
#[cold]
pub fn index_assignment_not_implemented() -> EvalError {
    EvalError::new("index assignment not yet implemented")
}

/// Field assignment not yet implemented.
#[cold]
pub fn field_assignment_not_implemented() -> EvalError {
    EvalError::new("field assignment not yet implemented")
}

/// Default requires type context.
#[cold]
pub fn default_requires_type_context() -> EvalError {
    EvalError::new("default() requires type context; use explicit construction instead")
}

// =============================================================================
// Index Context Errors
// =============================================================================

/// Operator not supported in index context.
#[cold]
pub fn operator_not_supported_in_index() -> EvalError {
    EvalError::new("operator not supported in index context")
}

/// Non-integer in index context.
#[cold]
pub fn non_integer_in_index() -> EvalError {
    EvalError::new("non-integer in index context")
}

/// Collection too large for indexing.
#[cold]
pub fn collection_too_large() -> EvalError {
    EvalError::new("collection too large")
}

// =============================================================================
// Pattern Errors
// =============================================================================

/// Unknown pattern kind.
#[cold]
pub fn unknown_pattern(kind: &str) -> EvalError {
    EvalError::new(format!("unknown pattern: {kind}"))
}

/// For pattern requires a list.
#[cold]
pub fn for_pattern_requires_list(actual: &str) -> EvalError {
    EvalError::new(format!("for pattern requires a list, got {actual}"))
}
