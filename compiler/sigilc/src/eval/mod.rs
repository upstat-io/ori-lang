//! Tree-walking interpreter for Sigil.
//!
//! This module provides runtime evaluation of Sigil expressions
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
//! methods on `Value`. See `sigil_patterns::value` for details.

mod evaluator;
mod output;
pub mod exec;
pub mod module;

/// Re-export Value types from `sigil_patterns` (single source of truth).
///
/// This module exists for import compatibility - files can continue using
/// `super::value::Value` instead of changing all imports.
pub mod value {
    pub use sigil_patterns::{
        Value, FunctionValue, RangeValue, StructValue, StructLayout, Heap, FunctionValFn,
    };
}

pub use value::{Value, FunctionValue, RangeValue, StructValue, StructLayout, Heap, FunctionValFn};
pub use sigil_eval::{
    Environment, LocalScope, Scope,
    evaluate_binary, evaluate_unary, dispatch_builtin_method,
    UserMethod, UserMethodRegistry,
};
pub use evaluator::{Evaluator, EvaluatorBuilder};
pub use sigil_patterns::{EvalResult, EvalError};
pub use output::{EvalOutput, ModuleEvalResult};

#[cfg(test)]
mod tests;
