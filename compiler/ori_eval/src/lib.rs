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

#![deny(clippy::arithmetic_side_effects)]
#![expect(
    clippy::result_large_err,
    reason = "EvalError is a fundamental type — boxing would add complexity across the crate"
)]

mod derives;
pub mod diagnostics;
mod environment;
pub mod errors;
mod eval_mode;
pub mod exec;
mod function_val;
pub mod interpreter;
mod method_key;
mod methods;
pub mod module_registration;
mod operators;
mod print_handler;
mod shared;
mod unary_operators;
mod user_methods;

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;

// Value types from ori_patterns — the natural API surface for consumers
pub use ori_patterns::{
    ControlAction, EvalError, EvalResult, FunctionValFn, FunctionValue, Heap,
    MemoizedFunctionValue, RangeValue, ScalarInt, StructLayout, StructValue, Value,
};

pub use diagnostics::{CallFrame, CallStack, EvalCounters};
pub use environment::{AssignError, Environment, LocalScope, Mutability, Scope};
pub use eval_mode::{BudgetExceeded, EvalMode, ModeState};
pub use method_key::MethodKey;
pub use methods::{dispatch_builtin_method_str, EVAL_BUILTIN_METHODS};
pub use operators::evaluate_binary;
pub use unary_operators::evaluate_unary;
pub use user_methods::{MethodEntry, UserMethod, UserMethodRegistry};

pub use derives::{process_derives, DefaultFieldTypeRegistry};
pub use function_val::{
    function_val_byte, function_val_float, function_val_int, function_val_str,
    function_val_thread_id,
};
pub use interpreter::resolvers::ITERATOR_METHOD_NAMES;
pub use interpreter::{Interpreter, InterpreterBuilder, ScopedInterpreter};
pub use ori_stack::ensure_sufficient_stack;
pub use print_handler::{
    buffer_handler, silent_handler, stdout_handler, BufferPrintHandler, PrintHandlerImpl,
    SharedPrintHandler, StdoutPrintHandler,
};
pub use shared::{SharedMutableRegistry, SharedRegistry};

// Re-export module registration functions for CLI and Playground
pub use module_registration::{
    collect_def_impl_methods_with_config, collect_extend_methods_with_config,
    collect_impl_methods_with_config, register_module_functions, register_newtype_constructors,
    register_variant_constructors, MethodCollectionConfig,
};
