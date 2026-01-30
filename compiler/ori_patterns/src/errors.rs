//! Error types for pattern evaluation.
//!
//! This module provides the core error types used during pattern evaluation
//! and the evaluator's runtime.

use crate::value::Value;
use ori_ir::Span;

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
    Continue,
    /// Return from a function, optionally with a value.
    Return(Value),
}

/// Evaluation error.
#[derive(Clone, Debug)]
pub struct EvalError {
    /// Error message.
    pub message: String,
    /// If this error is from `?` propagation, holds the original Err/None value.
    pub propagated_value: Option<Value>,
    /// If this is a control flow signal (break/continue/return), holds the signal.
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

    /// Create a continue signal.
    pub fn continue_signal() -> Self {
        EvalError {
            message: "continue".to_string(),
            propagated_value: None,
            control_flow: Some(ControlFlow::Continue),
            span: None,
        }
    }

    /// Create a return signal with a value.
    pub fn return_with(value: Value) -> Self {
        EvalError {
            message: format!("return:{value}"),
            propagated_value: None,
            control_flow: Some(ControlFlow::Return(value)),
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

/// Invalid operator for a specific type.
#[cold]
pub fn invalid_binary_op(type_name: &str) -> EvalError {
    EvalError::new(format!("invalid operator for {type_name}"))
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

// Method Call Errors

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

/// Map keys must be strings.
#[cold]
pub fn map_keys_must_be_strings() -> EvalError {
    EvalError::new("map keys must be strings")
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

#[cfg(test)]
// Tests check specific characters/numbers in error messages
#[allow(clippy::single_char_pattern, clippy::uninlined_format_args)]
mod tests {
    use super::*;

    // Test EvalError basic functionality
    #[test]
    fn test_eval_error_new() {
        let err = EvalError::new("test message");
        assert_eq!(err.message, "test message");
        assert!(err.propagated_value.is_none());
        assert!(err.control_flow.is_none());
    }

    #[test]
    fn test_eval_error_propagate() {
        let value = Value::int(42);
        let err = EvalError::propagate(value.clone(), "propagated error");
        assert_eq!(err.message, "propagated error");
        assert_eq!(err.propagated_value, Some(value));
        assert!(err.control_flow.is_none());
    }

    // Test ControlFlow enum
    #[test]
    fn test_control_flow_break() {
        let err = EvalError::break_with(Value::int(42));
        assert!(err.message.contains("break"));
        assert!(err.is_control_flow());
        assert_eq!(err.control_flow, Some(ControlFlow::Break(Value::int(42))));
    }

    #[test]
    fn test_control_flow_continue() {
        let err = EvalError::continue_signal();
        assert_eq!(err.message, "continue");
        assert!(err.is_control_flow());
        assert_eq!(err.control_flow, Some(ControlFlow::Continue));
    }

    #[test]
    fn test_control_flow_return() {
        let err = EvalError::return_with(Value::int(42));
        assert!(err.message.contains("return"));
        assert!(err.is_control_flow());
        assert_eq!(err.control_flow, Some(ControlFlow::Return(Value::int(42))));
    }

    #[test]
    fn test_is_control_flow() {
        let regular_err = EvalError::new("error");
        assert!(!regular_err.is_control_flow());

        let break_err = EvalError::break_with(Value::Void);
        assert!(break_err.is_control_flow());
    }

    // Binary Operation Errors
    #[test]
    fn test_invalid_binary_op() {
        let err = invalid_binary_op("string");
        assert!(err.message.contains("invalid operator"));
        assert!(err.message.contains("string"));
    }

    #[test]
    fn test_binary_type_mismatch() {
        let err = binary_type_mismatch("int", "string");
        assert!(err.message.contains("int"));
        assert!(err.message.contains("string"));
    }

    #[test]
    fn test_division_by_zero() {
        let err = division_by_zero();
        assert!(err.message.contains("division by zero"));
    }

    #[test]
    fn test_modulo_by_zero() {
        let err = modulo_by_zero();
        assert!(err.message.contains("modulo by zero"));
    }

    #[test]
    fn test_integer_overflow() {
        let err = integer_overflow("addition");
        assert!(err.message.contains("overflow"));
        assert!(err.message.contains("addition"));
    }

    // Method Call Errors
    #[test]
    fn test_no_such_method() {
        let err = no_such_method("foo", "int");
        assert!(err.message.contains("foo"));
        assert!(err.message.contains("int"));
    }

    #[test]
    fn test_wrong_arg_count() {
        let err = wrong_arg_count("map", 1, 2);
        assert!(err.message.contains("map"));
        assert!(err.message.contains("1"));
        assert!(err.message.contains("2"));
    }

    #[test]
    fn test_wrong_arg_type() {
        let err = wrong_arg_type("filter", "function");
        assert!(err.message.contains("filter"));
        assert!(err.message.contains("function"));
    }

    // Variable and Function Errors
    #[test]
    fn test_undefined_variable() {
        let err = undefined_variable("x");
        assert!(err.message.contains("undefined"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_undefined_function() {
        let err = undefined_function("foo");
        assert!(err.message.contains("undefined"));
        assert!(err.message.contains("@foo"));
    }

    #[test]
    fn test_undefined_config() {
        let err = undefined_config("PORT");
        assert!(err.message.contains("config"));
        assert!(err.message.contains("$PORT"));
    }

    #[test]
    fn test_not_callable() {
        let err = not_callable("int");
        assert!(err.message.contains("int"));
        assert!(err.message.contains("callable"));
    }

    #[test]
    fn test_wrong_function_args() {
        let err = wrong_function_args(2, 3);
        assert!(err.message.contains("2"));
        assert!(err.message.contains("3"));
    }

    // Index and Field Access Errors
    #[test]
    fn test_index_out_of_bounds() {
        let err = index_out_of_bounds(10);
        assert!(err.message.contains("10"));
        assert!(err.message.contains("bounds"));
    }

    #[test]
    fn test_key_not_found() {
        let err = key_not_found("missing");
        assert!(err.message.contains("missing"));
    }

    #[test]
    fn test_cannot_index() {
        let err = cannot_index("int", "string");
        assert!(err.message.contains("int"));
        assert!(err.message.contains("string"));
    }

    #[test]
    fn test_cannot_get_length() {
        let err = cannot_get_length("int");
        assert!(err.message.contains("int"));
        assert!(err.message.contains("length"));
    }

    #[test]
    fn test_no_field_on_struct() {
        let err = no_field_on_struct("missing");
        assert!(err.message.contains("missing"));
    }

    #[test]
    fn test_invalid_tuple_field() {
        let err = invalid_tuple_field("abc");
        assert!(err.message.contains("abc"));
    }

    #[test]
    fn test_tuple_index_out_of_bounds() {
        let err = tuple_index_out_of_bounds(5);
        assert!(err.message.contains("5"));
    }

    #[test]
    fn test_cannot_access_field() {
        let err = cannot_access_field("int");
        assert!(err.message.contains("int"));
    }

    #[test]
    fn test_no_member_in_module() {
        let err = no_member_in_module("foo");
        assert!(err.message.contains("foo"));
        assert!(err.message.contains("module"));
    }

    // Type Conversion Errors
    #[test]
    fn test_range_bound_not_int() {
        let err = range_bound_not_int("start");
        assert!(err.message.contains("start"));
        assert!(err.message.contains("integer"));
    }

    #[test]
    fn test_unbounded_range_end() {
        let err = unbounded_range_end();
        assert!(err.message.contains("unbounded"));
    }

    #[test]
    fn test_map_keys_must_be_strings() {
        let err = map_keys_must_be_strings();
        assert!(err.message.contains("strings"));
    }

    // Control Flow Errors
    #[test]
    fn test_non_exhaustive_match() {
        let err = non_exhaustive_match();
        assert!(err.message.contains("non-exhaustive"));
    }

    #[test]
    fn test_cannot_assign_immutable() {
        let err = cannot_assign_immutable("x");
        assert!(err.message.contains("immutable"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn test_invalid_assignment_target() {
        let err = invalid_assignment_target();
        assert!(err.message.contains("assignment"));
    }

    #[test]
    fn test_for_requires_iterable() {
        let err = for_requires_iterable();
        assert!(err.message.contains("iterable"));
    }

    // Pattern Binding Errors
    #[test]
    fn test_tuple_pattern_mismatch() {
        let err = tuple_pattern_mismatch();
        assert!(err.message.contains("tuple"));
        assert!(err.message.contains("mismatch"));
    }

    #[test]
    fn test_expected_tuple() {
        let err = expected_tuple();
        assert!(err.message.contains("tuple"));
    }

    #[test]
    fn test_expected_struct() {
        let err = expected_struct();
        assert!(err.message.contains("struct"));
    }

    #[test]
    fn test_expected_list() {
        let err = expected_list();
        assert!(err.message.contains("list"));
    }

    #[test]
    fn test_list_pattern_too_long() {
        let err = list_pattern_too_long();
        assert!(err.message.contains("list"));
        assert!(err.message.contains("too long"));
    }

    #[test]
    fn test_missing_struct_field() {
        let err = missing_struct_field();
        assert!(err.message.contains("struct"));
        assert!(err.message.contains("field"));
    }

    // Miscellaneous Errors
    #[test]
    fn test_self_outside_method() {
        let err = self_outside_method();
        assert!(err.message.contains("self"));
    }

    #[test]
    fn test_parse_error() {
        let err = parse_error();
        assert!(err.message.contains("parse"));
    }

    #[test]
    fn test_hash_outside_index() {
        let err = hash_outside_index();
        assert!(err.message.contains("#"));
    }

    #[test]
    fn test_await_not_supported() {
        let err = await_not_supported();
        assert!(err.message.contains("await"));
    }

    #[test]
    fn test_invalid_literal_pattern() {
        let err = invalid_literal_pattern();
        assert!(err.message.contains("literal"));
    }

    // Collection Method Errors
    #[test]
    fn test_map_requires_collection() {
        let err = map_requires_collection();
        assert!(err.message.contains("map"));
        assert!(err.message.contains("collection"));
    }

    #[test]
    fn test_filter_requires_collection() {
        let err = filter_requires_collection();
        assert!(err.message.contains("filter"));
        assert!(err.message.contains("collection"));
    }

    #[test]
    fn test_fold_requires_collection() {
        let err = fold_requires_collection();
        assert!(err.message.contains("fold"));
        assert!(err.message.contains("collection"));
    }

    #[test]
    fn test_find_requires_list() {
        let err = find_requires_list();
        assert!(err.message.contains("find"));
        assert!(err.message.contains("list"));
    }

    #[test]
    fn test_collect_requires_range() {
        let err = collect_requires_range();
        assert!(err.message.contains("collect"));
        assert!(err.message.contains("range"));
    }

    #[test]
    fn test_any_requires_list() {
        let err = any_requires_list();
        assert!(err.message.contains("any"));
        assert!(err.message.contains("list"));
    }

    #[test]
    fn test_all_requires_list() {
        let err = all_requires_list();
        assert!(err.message.contains("all"));
        assert!(err.message.contains("list"));
    }

    #[test]
    fn test_map_entries_requires_map() {
        let err = map_entries_requires_map();
        assert!(err.message.contains("map"));
    }

    #[test]
    fn test_filter_entries_requires_map() {
        let err = filter_entries_requires_map();
        assert!(err.message.contains("filter"));
        assert!(err.message.contains("map"));
    }

    // Not Implemented Errors
    #[test]
    fn test_map_entries_not_implemented() {
        let err = map_entries_not_implemented();
        assert!(err.message.contains("not yet"));
    }

    #[test]
    fn test_filter_entries_not_implemented() {
        let err = filter_entries_not_implemented();
        assert!(err.message.contains("not yet"));
    }

    #[test]
    fn test_index_assignment_not_implemented() {
        let err = index_assignment_not_implemented();
        assert!(err.message.contains("not yet"));
    }

    #[test]
    fn test_field_assignment_not_implemented() {
        let err = field_assignment_not_implemented();
        assert!(err.message.contains("not yet"));
    }

    #[test]
    fn test_default_requires_type_context() {
        let err = default_requires_type_context();
        assert!(err.message.contains("default"));
        assert!(err.message.contains("type"));
    }

    // Index Context Errors
    #[test]
    fn test_operator_not_supported_in_index() {
        let err = operator_not_supported_in_index();
        assert!(err.message.contains("operator"));
        assert!(err.message.contains("index"));
    }

    #[test]
    fn test_non_integer_in_index() {
        let err = non_integer_in_index();
        assert!(err.message.contains("integer"));
        assert!(err.message.contains("index"));
    }

    #[test]
    fn test_collection_too_large() {
        let err = collection_too_large();
        assert!(err.message.contains("collection"));
        assert!(err.message.contains("large"));
    }

    // Pattern Errors
    #[test]
    fn test_unknown_pattern() {
        let err = unknown_pattern("weird");
        assert!(err.message.contains("weird"));
    }

    #[test]
    fn test_for_pattern_requires_list() {
        let err = for_pattern_requires_list("int");
        assert!(err.message.contains("for"));
        assert!(err.message.contains("list"));
        assert!(err.message.contains("int"));
    }

    // Propagation Helpers
    #[test]
    fn test_propagated_error_message() {
        let msg = propagated_error_message(&Value::int(42));
        assert!(msg.contains("propagated"));
        assert!(msg.contains("42"));
    }

    // Test that errors are distinct (different messages)
    #[test]
    fn test_errors_are_distinct() {
        let errors = vec![
            division_by_zero().message,
            modulo_by_zero().message,
            non_exhaustive_match().message,
            invalid_assignment_target().message,
            for_requires_iterable().message,
            tuple_pattern_mismatch().message,
            expected_tuple().message,
            expected_struct().message,
            expected_list().message,
            list_pattern_too_long().message,
            missing_struct_field().message,
            self_outside_method().message,
            no_member_in_module("test").message,
            parse_error().message,
            hash_outside_index().message,
            await_not_supported().message,
            invalid_literal_pattern().message,
            map_requires_collection().message,
            filter_requires_collection().message,
            fold_requires_collection().message,
            find_requires_list().message,
            collect_requires_range().message,
            any_requires_list().message,
            all_requires_list().message,
            unbounded_range_end().message,
            map_keys_must_be_strings().message,
        ];

        // Ensure all messages are unique
        let mut seen = std::collections::HashSet::new();
        for msg in &errors {
            assert!(seen.insert(msg.clone()), "Duplicate error message: {}", msg);
        }
    }
}
