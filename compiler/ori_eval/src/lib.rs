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
pub mod module_registration;
mod operators;
mod print_handler;
mod shared;
mod stack;
mod unary_operators;
mod user_methods;

// Re-export value types from ori_patterns
pub use ori_patterns::{
    EvalContext, EvalError, EvalResult, FunctionValFn, FunctionValue, Heap, MemoizedFunctionValue,
    PatternDefinition, PatternExecutor, PatternRegistry, RangeValue, ScalarInt, StructLayout,
    StructValue, TypeCheckContext, Value,
};

// Re-export error constructors for convenience (canonical path is ori_eval::errors::*)
pub use errors::{
    // Collection method errors
    all_requires_list,
    any_requires_list,
    // Miscellaneous errors
    await_not_supported,
    // Binary operation errors
    binary_type_mismatch,
    // Index and field access errors
    cannot_access_field,
    // Control flow errors
    cannot_assign_immutable,
    cannot_get_length,
    cannot_index,
    collect_requires_range,
    // Index context errors
    collection_too_large,
    // Not implemented errors
    default_requires_type_context,
    division_by_zero,
    // Pattern binding errors
    expected_list,
    expected_struct,
    expected_tuple,
    field_assignment_not_implemented,
    filter_entries_not_implemented,
    filter_entries_requires_map,
    filter_requires_collection,
    find_requires_list,
    fold_requires_collection,
    // Pattern errors
    for_pattern_requires_list,
    for_requires_iterable,
    hash_outside_index,
    index_assignment_not_implemented,
    index_out_of_bounds,
    invalid_assignment_target,
    invalid_binary_op,
    invalid_literal_pattern,
    invalid_tuple_field,
    key_not_found,
    list_pattern_too_long,
    map_entries_not_implemented,
    map_entries_requires_map,
    // Type conversion errors
    map_keys_must_be_strings,
    map_requires_collection,
    missing_struct_field,
    modulo_by_zero,
    no_field_on_struct,
    no_member_in_module,
    // Method call errors
    no_such_method,
    non_exhaustive_match,
    non_integer_in_index,
    // Variable and function errors
    not_callable,
    operator_not_supported_in_index,
    parse_error,
    range_bound_not_int,
    self_outside_method,
    tuple_index_out_of_bounds,
    tuple_pattern_mismatch,
    unbounded_range_end,
    undefined_config,
    undefined_function,
    undefined_variable,
    unknown_pattern,
    wrong_arg_count,
    wrong_arg_type,
    wrong_function_args,
};

pub use environment::{Environment, LocalScope, Mutability, Scope};
pub use method_key::MethodKey;
pub use methods::{dispatch_builtin_method, EVAL_BUILTIN_METHODS};
pub use operators::evaluate_binary;
pub use unary_operators::evaluate_unary;
pub use user_methods::{MethodEntry, UserMethod, UserMethodRegistry};
// Re-export from ori_ir for backward compatibility
pub use ori_ir::{DerivedMethodInfo, DerivedTrait};

pub use function_val::{
    function_val_byte, function_val_float, function_val_int, function_val_str,
    function_val_thread_id,
};
pub use interpreter::{Interpreter, InterpreterBuilder};
pub use print_handler::{
    buffer_handler, stdout_handler, BufferPrintHandler, PrintHandlerImpl, SharedPrintHandler,
    StdoutPrintHandler,
};
pub use shared::{SharedMutableRegistry, SharedRegistry};
pub use stack::ensure_sufficient_stack;

// Re-export module registration functions for CLI and Playground
pub use module_registration::{
    collect_extend_methods, collect_impl_methods, register_module_functions,
    register_newtype_constructors, register_variant_constructors,
};
