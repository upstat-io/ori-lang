//! Error types for pattern evaluation.
//!
//! This module provides the core error types used during pattern evaluation
//! and the evaluator's runtime.

use crate::value::Value;
use ori_ir::{BinaryOp, Span};

/// Result of evaluation.
pub type EvalResult = Result<Value, EvalError>;

/// Control flow signals for break, continue, and return.
///
/// These are not errors but signals that need to be propagated up the call stack
/// to the appropriate handler (loop, function boundary).
#[derive(Clone, Debug, PartialEq)]
pub enum ControlFlow {
    /// Break from a loop, optionally with a value.
    Break(Value),
    /// Continue to the next iteration of a loop.
    /// In `for...yield` context, may carry a substitution value.
    Continue(Value),
}

/// Evaluation error.
#[derive(Clone, Debug)]
pub struct EvalError {
    /// Error message.
    pub message: String,
    /// If this error is from `?` propagation, holds the original Err/None value.
    pub propagated_value: Option<Value>,
    /// If this is a control flow signal (break/continue), holds the signal.
    pub control_flow: Option<ControlFlow>,
    /// Source location where the error occurred.
    ///
    /// This enables better error messages with file/line/column information.
    /// Patterns should attach spans when creating errors from property evaluation.
    pub span: Option<Span>,
}

impl EvalError {
    pub fn new(message: impl Into<String>) -> Self {
        EvalError {
            message: message.into(),
            propagated_value: None,
            control_flow: None,
            span: None,
        }
    }

    /// Create an error for propagating an Err or None value.
    pub fn propagate(value: Value, message: impl Into<String>) -> Self {
        EvalError {
            message: message.into(),
            propagated_value: Some(value),
            control_flow: None,
            span: None,
        }
    }

    /// Create a break signal with a value.
    pub fn break_with(value: Value) -> Self {
        EvalError {
            message: format!("break:{value}"),
            propagated_value: None,
            control_flow: Some(ControlFlow::Break(value)),
            span: None,
        }
    }

    /// Create a continue signal without a substitution value.
    pub fn continue_signal() -> Self {
        EvalError {
            message: "continue".to_string(),
            propagated_value: None,
            control_flow: Some(ControlFlow::Continue(Value::Void)),
            span: None,
        }
    }

    /// Create a continue signal with a substitution value (for `for...yield`).
    pub fn continue_with(value: Value) -> Self {
        EvalError {
            message: format!("continue:{value}"),
            propagated_value: None,
            control_flow: Some(ControlFlow::Continue(value)),
            span: None,
        }
    }

    /// Attach a source span to this error.
    ///
    /// This is a builder method that returns the modified error,
    /// enabling chained construction: `EvalError::new("msg").with_span(span)`.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Check if this error is a control flow signal.
    #[inline]
    pub fn is_control_flow(&self) -> bool {
        self.control_flow.is_some()
    }
}

// Binary Operation Errors

/// Invalid operator for a specific type with operator context.
///
/// Provides better error messages by including the specific operator that failed.
#[cold]
pub fn invalid_binary_op_for(type_name: &str, op: BinaryOp) -> EvalError {
    EvalError::new(format!(
        "operator `{}` cannot be applied to {type_name}",
        op.as_symbol()
    ))
}

/// Type mismatch in binary operation.
#[cold]
pub fn binary_type_mismatch(left: &str, right: &str) -> EvalError {
    EvalError::new(format!("cannot apply operator to `{left}` and `{right}`"))
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

/// Integer overflow error.
#[cold]
pub fn integer_overflow(operation: &str) -> EvalError {
    EvalError::new(format!("integer overflow in {operation}"))
}

/// Maximum recursion depth exceeded error.
///
/// Used on WASM to prevent stack exhaustion with a clear error message
/// instead of a cryptic "memory access out of bounds".
#[cold]
pub fn recursion_limit_exceeded(limit: usize) -> EvalError {
    EvalError::new(format!(
        "maximum recursion depth exceeded (WASM limit: {limit})"
    ))
}

// Method Call Errors

/// No such method on a type.
#[cold]
pub fn no_such_method(method: &str, type_name: &str) -> EvalError {
    EvalError::new(format!("no method '{method}' on type {type_name}"))
}

/// Wrong argument count for a method.
#[cold]
pub fn wrong_arg_count(method: &str, expected: usize, got: usize) -> EvalError {
    let arg_word = if expected == 1 {
        "argument"
    } else {
        "arguments"
    };
    EvalError::new(format!("{method} expects {expected} {arg_word}, got {got}"))
}

/// Wrong argument type for a method.
#[cold]
pub fn wrong_arg_type(method: &str, expected: &str) -> EvalError {
    EvalError::new(format!("{method} expects a {expected} argument"))
}

// Variable and Function Errors

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

/// Undefined constant.
#[cold]
pub fn undefined_const(name: &str) -> EvalError {
    EvalError::new(format!("undefined constant: ${name}"))
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

// Index and Field Access Errors

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

/// No member in module namespace.
#[cold]
pub fn no_member_in_module(member: &str) -> EvalError {
    EvalError::new(format!("module has no member '{member}'"))
}

// Type Conversion and Validation Errors

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

/// Map keys must be hashable types (primitives, tuples of hashables).
#[cold]
pub fn map_key_not_hashable() -> EvalError {
    EvalError::new("map keys must be hashable (primitives, tuples, etc.)")
}

/// Spread requires a map value.
#[cold]
pub fn spread_requires_map() -> EvalError {
    EvalError::new("spread in map literal requires a map value")
}

// Control Flow Errors

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

// Pattern Binding Errors

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

// Miscellaneous Errors

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

// Collection Method Errors

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

// Not Implemented Errors

/// Map entries not yet implemented.
#[cold]
pub fn map_entries_not_implemented() -> EvalError {
    EvalError::new(
        "map_entries() is not yet implemented; use map() with entry destructuring: \
         map.entries().map((k, v) -> ...)",
    )
}

/// Filter entries not yet implemented.
#[cold]
pub fn filter_entries_not_implemented() -> EvalError {
    EvalError::new(
        "filter_entries() is not yet implemented; use filter() with entry destructuring: \
         map.entries().filter((k, v) -> ...)",
    )
}

/// Index assignment not yet implemented.
#[cold]
pub fn index_assignment_not_implemented() -> EvalError {
    EvalError::new(
        "index assignment (list[i] = x) is not yet implemented; \
         use list.set(index: i, value: x) instead",
    )
}

/// Field assignment not yet implemented.
#[cold]
pub fn field_assignment_not_implemented() -> EvalError {
    EvalError::new(
        "field assignment (obj.field = x) is not yet implemented; \
         use spread syntax: { ...obj, field: x }",
    )
}

/// Default requires type context.
#[cold]
pub fn default_requires_type_context() -> EvalError {
    EvalError::new("default() requires type context; use explicit construction instead")
}

// Index Context Errors

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

// Pattern Errors

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

// Propagation Helpers

/// Create a standardized propagated error message.
///
/// This ensures consistent formatting of propagated errors across all call sites.
#[cold]
pub fn propagated_error_message(value: &Value) -> String {
    format!("propagated error: {value:?}")
}
