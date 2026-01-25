//! Sigil Eval - Interpreter/evaluator for the Sigil compiler.
//!
//! This crate provides the tree-walking interpreter for Sigil programs.
//!
//! # Architecture
//!
//! The evaluator uses:
//! - `Environment`: Variable scoping with a scope stack
//! - `OperatorRegistry`: Binary operator dispatch
//! - `Value` types from `sigil_patterns`
//!
//! # Re-exports
//!
//! This crate re-exports value types from `sigil_patterns` for convenience:
//! - `Value`, `FunctionValue`, `RangeValue`, `StructValue`, `StructLayout`, `Heap`
//! - `EvalError`, `EvalResult`

mod environment;
mod operators;

// Re-export value types from sigil_patterns
pub use sigil_patterns::{
    EvalContext, EvalError, EvalResult, FunctionValFn, FunctionValue, Heap, PatternDefinition,
    PatternExecutor, PatternRegistry, RangeValue, SharedPattern, StructLayout, StructValue,
    TypeCheckContext, Value,
};

// Re-export error constructors from sigil_patterns
pub use sigil_patterns::{
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
pub use operators::{BinaryOperator, OperatorRegistry};
