//! Tree-walking interpreter for Ori.
//!
//! This module provides runtime evaluation of Ori expressions
//! using a Salsa-compatible AST representation.
//!
//! ## Architecture
//!
//! - `Value`: Runtime values with enforced Arc usage through factory methods
//! - `Environment`: Variable scoping with stack-based lookup
//! - `Evaluator`: Tree-walking interpreter
//!
//! ## Arc Enforcement
//!
//! The value module enforces that all heap allocations go through factory
//! methods on `Value`. See `ori_patterns::value` for details.

mod evaluator;
pub mod module;
mod output;

// Re-export exec module from ori_eval
pub use ori_eval::exec;

/// Re-export Value types from `ori_patterns` (single source of truth).
///
/// This module exists for import compatibility - files can continue using
/// `super::value::Value` instead of changing all imports.
pub mod value {
    pub use ori_patterns::{
        FunctionValFn, FunctionValue, Heap, RangeValue, StructLayout, StructValue, Value,
    };
}

pub use evaluator::{Evaluator, EvaluatorBuilder, ScopedEvaluator};
pub use ori_eval::{
    dispatch_builtin_method, evaluate_binary, evaluate_unary, Environment, LocalScope, Mutability,
    Scope, UserMethod, UserMethodRegistry,
};
pub use ori_patterns::{EvalError, EvalResult};
pub use output::{EvalOutput, ModuleEvalResult};
pub use value::{FunctionValFn, FunctionValue, Heap, RangeValue, StructLayout, StructValue, Value};

#[cfg(test)]
mod tests;
