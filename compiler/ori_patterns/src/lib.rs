#![deny(clippy::arithmetic_side_effects)]
#![allow(
    clippy::result_large_err,
    reason = "EvalError is fundamental — boxing would add complexity across the crate"
)]
//! Ori Patterns - Pattern system for the Ori compiler.
//!
//! This crate provides:
//! - Runtime value types (`Value`, `Heap`, `FunctionValue`, `RangeValue`, etc.)
//! - Evaluation error types (`EvalError`, `EvalResult`)
//! - Pattern registry and trait definitions
//! - Built-in pattern implementations (recurse, parallel, spawn, timeout, cache, with)
//!
//! # Architecture
//!
//! The pattern system follows the Open/Closed principle:
//! - New patterns can be added by implementing `PatternDefinition`
//! - No modifications to existing code required
//! - Patterns are registered in `PatternRegistry`
//!
//! # Value Types
//!
//! The value module provides runtime values with enforced Arc usage:
//! - All heap allocations go through `Value::` factory methods
//! - `Heap<T>` wrapper enforces this invariant
//! - Thread-safe reference counting via `Arc`

mod context;
mod errors;
mod executor;
mod fusion;
mod iterable;
pub mod method_key;
mod pattern_def;
mod registry;
mod signature;
pub mod user_methods;
mod value;

// Pattern implementations
mod builtins;
mod cache;
mod channel;
mod parallel;
mod recurse;
mod spawn;
mod timeout;
mod with_pattern;

#[cfg(test)]
mod parallel_tests;

#[cfg(test)]
mod test_helpers;

pub use context::EvalContext;
pub use executor::{EvalAction, PatternExecutor};
pub use iterable::{Iterable, IterableIter};
pub use pattern_def::{
    PatternCore, PatternDefinition, PatternFusable, PatternVariadic, ScopedBinding,
    ScopedBindingType,
};

pub use errors::{
    BacktraceFrame, ControlAction, EvalBacktrace, EvalError, EvalErrorKind, EvalNote, EvalResult,
};
pub use fusion::{ChainLink, FusedPattern, FusionHints, PatternChain};
pub use method_key::{MethodKey, MethodKeyDisplay};
pub use registry::{Pattern, PatternRegistry};
pub use signature::{DefaultValue, FunctionSignature, OptionalArg, PatternSignature};
pub use user_methods::{MethodEntry, UserMethod, UserMethodRegistry};
pub use value::{
    ErrorValue, FunctionValFn, FunctionValue, Heap, IteratorValue, ListData, MapData,
    MemoizedFunctionValue, OrderingValue, RangeValue, ScalarInt, StringLookup, StructLayout,
    StructValue, TraceEntryData, Value,
};

// Re-export error constructors for use by other crates
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
    index_assignment_not_supported,
    index_out_of_bounds,
    integer_overflow,
    invalid_assignment_target,
    invalid_binary_op_for,
    invalid_literal_pattern,
    invalid_tuple_field,
    join_requires_list,
    key_not_found,
    list_pattern_too_long,
    map_entries_not_implemented,
    map_entries_requires_map,
    // Type conversion errors
    map_key_not_hashable,
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
    propagated_error_message,
    range_bound_not_int,
    recursion_limit_exceeded,
    self_outside_method,
    size_negative_divide,
    size_negative_multiply,
    size_would_be_negative,
    spread_requires_list,
    spread_requires_map,
    spread_requires_struct,
    tuple_index_out_of_bounds,
    tuple_pattern_mismatch,
    unbounded_range_eager,
    unbounded_range_length,
    undefined_const,
    undefined_function,
    undefined_variable,
    unknown_pattern,
    wrong_arg_count,
    wrong_arg_type,
    wrong_function_args,
};
