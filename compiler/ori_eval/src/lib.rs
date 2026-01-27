#![deny(clippy::arithmetic_side_effects)]
//! Ori Eval - Interpreter/evaluator for the Ori compiler.
//!
//! This crate provides the tree-walking interpreter for Ori programs.
//!
//! # Architecture
//!
//! The evaluator uses:
//! - `Environment`: Variable scoping with a scope stack
//! - `evaluate_binary`: Direct enum-based binary operator dispatch
//! - `evaluate_unary`: Direct enum-based unary operator dispatch
//! - `dispatch_builtin_method`: Direct enum-based method dispatch for built-in types
//! - `UserMethodRegistry`: User-defined method dispatch for impl blocks
//! - `Value` types from `ori_patterns`
//!
//! # Re-exports
//!
//! This crate re-exports value types from `ori_patterns` for convenience:
//! - `Value`, `FunctionValue`, `RangeValue`, `StructValue`, `StructLayout`, `Heap`
//! - `EvalError`, `EvalResult`

mod environment;
pub mod errors;
pub mod exec;
mod function_val;
pub mod interpreter;
mod method_key;
mod methods;
mod operators;
mod print_handler;
mod shared;
mod stack;
mod unary_operators;
mod user_methods;

// Re-export value types from ori_patterns
pub use ori_patterns::{
    EvalContext, EvalError, EvalResult, FunctionValFn, FunctionValue, Heap, MemoizedFunctionValue,
    PatternDefinition, PatternExecutor, PatternRegistry, RangeValue, ScalarInt, SharedPattern,
    StructLayout, StructValue, TypeCheckContext, Value,
};

// Re-export error constructors for convenience (canonical path is ori_eval::errors::*)
pub use errors::{
    // Binary operation errors
    binary_type_mismatch, division_by_zero, invalid_binary_op, modulo_by_zero,
    // Method call errors
    no_such_method, wrong_arg_count, wrong_arg_type,
    // Variable and function errors
    not_callable, undefined_config, undefined_function, undefined_variable, wrong_function_args,
    // Index and field access errors
    cannot_access_field, cannot_get_length, cannot_index, index_out_of_bounds,
    invalid_tuple_field, key_not_found, no_field_on_struct, tuple_index_out_of_bounds,
    // Type conversion errors
    map_keys_must_be_strings, range_bound_not_int, unbounded_range_end,
    // Control flow errors
    cannot_assign_immutable, for_requires_iterable, invalid_assignment_target, non_exhaustive_match,
    // Pattern binding errors
    expected_list, expected_struct, expected_tuple, list_pattern_too_long,
    missing_struct_field, tuple_pattern_mismatch,
    // Miscellaneous errors
    await_not_supported, hash_outside_index, invalid_literal_pattern, parse_error, self_outside_method,
    // Collection method errors
    all_requires_list, any_requires_list, collect_requires_range, filter_entries_requires_map,
    filter_requires_collection, find_requires_list, fold_requires_collection,
    map_entries_requires_map, map_requires_collection,
    // Not implemented errors
    default_requires_type_context, field_assignment_not_implemented,
    filter_entries_not_implemented, index_assignment_not_implemented, map_entries_not_implemented,
    // Index context errors
    collection_too_large, non_integer_in_index, operator_not_supported_in_index,
    // Pattern errors
    for_pattern_requires_list, unknown_pattern,
};

pub use environment::{Environment, LocalScope, Scope};
pub use methods::dispatch_builtin_method;
pub use operators::evaluate_binary;
pub use unary_operators::evaluate_unary;
pub use method_key::MethodKey;
pub use user_methods::{MethodEntry, UserMethod, UserMethodRegistry};
// Re-export from ori_ir for backward compatibility
pub use ori_ir::{DerivedMethodInfo, DerivedTrait};

pub use function_val::{
    function_val_byte, function_val_float, function_val_int, function_val_str,
    function_val_thread_id,
};
pub use shared::{SharedRegistry, SharedMutableRegistry};
pub use stack::ensure_sufficient_stack;
pub use interpreter::{Interpreter, InterpreterBuilder};
pub use print_handler::{
    PrintHandler, StdoutPrintHandler, BufferPrintHandler, SharedPrintHandler,
    stdout_handler, buffer_handler,
};
