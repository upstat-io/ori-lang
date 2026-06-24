//! Error-factory functions for arithmetic, type/operator, access, function,
//! control-flow, and pattern-binding evaluation failures.

use ori_ir::BinaryOp;

use crate::errors::{EvalError, EvalErrorKind};

// Binary Operation Errors

/// Invalid operator for a specific type with operator context.
#[cold]
pub fn invalid_binary_op_for(type_name: &str, op: BinaryOp) -> EvalError {
    EvalError::from_kind(EvalErrorKind::InvalidBinaryOp {
        type_name: type_name.to_string(),
        op,
    })
}

/// Type mismatch in binary operation.
#[cold]
pub fn binary_type_mismatch(left: &str, right: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::BinaryTypeMismatch {
        left: left.to_string(),
        right: right.to_string(),
    })
}

/// Division by zero error.
#[cold]
pub fn division_by_zero() -> EvalError {
    EvalError::from_kind(EvalErrorKind::DivisionByZero)
}

/// Modulo by zero error.
#[cold]
pub fn modulo_by_zero() -> EvalError {
    EvalError::from_kind(EvalErrorKind::ModuloByZero)
}

/// Integer overflow error.
#[cold]
pub fn integer_overflow(operation: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::IntegerOverflow {
        operation: operation.to_string(),
    })
}

/// Size subtraction would produce a negative result.
#[cold]
pub fn size_would_be_negative() -> EvalError {
    EvalError::from_kind(EvalErrorKind::SizeWouldBeNegative)
}

/// Cannot multiply Size by a negative integer.
#[cold]
pub fn size_negative_multiply() -> EvalError {
    EvalError::from_kind(EvalErrorKind::SizeNegativeMultiply)
}

/// Cannot divide Size by a negative integer.
#[cold]
pub fn size_negative_divide() -> EvalError {
    EvalError::from_kind(EvalErrorKind::SizeNegativeDivide)
}

/// Maximum recursion depth exceeded error.
#[cold]
pub fn recursion_limit_exceeded(limit: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::StackOverflow { depth: limit })
}

// Method Call Errors

/// No such method on a type.
#[cold]
pub fn no_such_method(method: &str, type_name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedMethod {
        method: method.to_string(),
        type_name: type_name.to_string(),
    })
}

/// Wrong argument count for a method.
#[cold]
pub fn wrong_arg_count(method: &str, expected: usize, got: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::ArityMismatch {
        name: method.to_string(),
        expected,
        got,
    })
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
    EvalError::from_kind(EvalErrorKind::UndefinedVariable {
        name: name.to_string(),
    })
}

/// Undefined function.
#[cold]
pub fn undefined_function(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedFunction {
        name: name.to_string(),
    })
}

/// Undefined constant.
#[cold]
pub fn undefined_const(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::UndefinedConst {
        name: name.to_string(),
    })
}

/// Value is not callable.
#[cold]
pub fn not_callable(type_name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::NotCallable {
        type_name: type_name.to_string(),
    })
}

/// Wrong number of arguments in function call.
#[cold]
pub fn wrong_function_args(expected: usize, got: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::ArityMismatch {
        name: String::new(),
        expected,
        got,
    })
}

// Index and Field Access Errors

/// Index out of bounds.
#[cold]
pub fn index_out_of_bounds(index: i64) -> EvalError {
    EvalError::from_kind(EvalErrorKind::IndexOutOfBounds { index })
}

/// Key not found in map.
#[cold]
pub fn key_not_found(key: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::KeyNotFound {
        key: key.to_string(),
    })
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
    EvalError::from_kind(EvalErrorKind::UndefinedField {
        field: field.to_string(),
    })
}

/// Invalid tuple field.
#[cold]
pub fn invalid_tuple_field(field: &str) -> EvalError {
    EvalError::new(format!("invalid tuple field: {field}"))
}

/// Tuple index out of bounds.
#[cold]
pub fn tuple_index_out_of_bounds(index: usize) -> EvalError {
    EvalError::from_kind(EvalErrorKind::IndexOutOfBounds {
        index: i64::try_from(index).unwrap_or(i64::MAX),
    })
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

/// Cannot compute length of unbounded range.
#[cold]
pub fn unbounded_range_length() -> EvalError {
    EvalError::new("cannot compute length of unbounded range")
}

/// Cannot eagerly consume an unbounded range.
#[cold]
pub fn unbounded_range_eager(method: &str) -> EvalError {
    EvalError::new(format!(
        "cannot {method}() an unbounded range; use .iter().take(n).{method}() instead"
    ))
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

/// Spread requires a list value.
#[cold]
pub fn spread_requires_list() -> EvalError {
    EvalError::new("spread in list literal requires a list value")
}

/// Spread requires a struct value.
#[cold]
pub fn spread_requires_struct() -> EvalError {
    EvalError::new("spread in struct literal requires a struct value")
}

// Control Flow Errors

/// Non-exhaustive match.
#[cold]
pub fn non_exhaustive_match() -> EvalError {
    EvalError::from_kind(EvalErrorKind::NonExhaustiveMatch)
}

/// Cannot assign to immutable variable.
#[cold]
pub fn cannot_assign_immutable(name: &str) -> EvalError {
    EvalError::from_kind(EvalErrorKind::ImmutableBinding {
        name: name.to_string(),
    })
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
