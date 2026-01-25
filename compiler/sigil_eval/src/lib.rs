//! Sigil Eval - Interpreter/evaluator for the Sigil compiler.
//!
//! This crate provides the tree-walking interpreter for Sigil programs.
//!
//! # Architecture
//!
//! The evaluator uses:
//! - `Environment`: Variable scoping with a scope stack
//! - `OperatorRegistry`: Binary operator dispatch
//! - `UnaryOperatorRegistry`: Unary operator dispatch
//! - `MethodRegistry`: Method dispatch for built-in types
//! - `UserMethodRegistry`: User-defined method dispatch for impl blocks
//! - `Value` types from `sigil_patterns`
//!
//! # Re-exports
//!
//! This crate re-exports value types from `sigil_patterns` for convenience:
//! - `Value`, `FunctionValue`, `RangeValue`, `StructValue`, `StructLayout`, `Heap`
//! - `EvalError`, `EvalResult`

mod environment;
pub mod errors;
mod function_val;
mod methods;
mod operators;
mod unary_operators;
mod user_methods;

// Re-export value types from sigil_patterns
pub use sigil_patterns::{
    EvalContext, EvalError, EvalResult, FunctionValFn, FunctionValue, Heap, PatternDefinition,
    PatternExecutor, PatternRegistry, RangeValue, SharedPattern, StructLayout, StructValue,
    TypeCheckContext, Value,
};

// Re-export error constructors for convenience (canonical path is sigil_eval::errors::*)
pub use errors::{
    await_not_supported, binary_type_mismatch, cannot_access_field, cannot_assign_immutable,
    cannot_get_length, cannot_index, division_by_zero, expected_list, expected_struct,
    expected_tuple, for_requires_iterable, hash_outside_index, index_out_of_bounds,
    invalid_assignment_target, invalid_binary_op, invalid_literal_pattern, invalid_tuple_field,
    key_not_found, list_pattern_too_long, map_keys_must_be_strings, missing_struct_field,
    modulo_by_zero, no_field_on_struct, no_such_method, non_exhaustive_match, not_callable,
    parse_error, range_bound_not_int, self_outside_method, tuple_index_out_of_bounds,
    tuple_pattern_mismatch, unbounded_range_end, undefined_config, undefined_function,
    undefined_variable, wrong_arg_count, wrong_arg_type, wrong_function_args,
};

pub use environment::{Environment, LocalScope, Scope};
pub use methods::{MethodDispatcher, MethodRegistry};
pub use operators::{BinaryOperator, OperatorRegistry};
pub use unary_operators::{UnaryOperator, UnaryOperatorRegistry};
pub use user_methods::{
    DerivedMethodInfo, DerivedTrait, MethodEntry, UserMethod, UserMethodRegistry,
};

pub use function_val::{
    function_val_byte, function_val_float, function_val_int, function_val_str,
    function_val_thread_id,
};
