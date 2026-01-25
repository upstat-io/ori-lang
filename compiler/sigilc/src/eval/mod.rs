//! Tree-walking interpreter for Sigil V3.
//!
//! This module provides runtime evaluation of Sigil expressions.
//! Ported from V2 but adapted to work with V3's Salsa-compatible AST.
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

mod environment;
mod evaluator;
mod output;
mod function_val;
pub mod errors;
pub mod operators;
pub mod methods;
pub mod user_methods;
pub mod unary_operators;
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
pub use environment::Environment;
pub use evaluator::{Evaluator, EvaluatorBuilder, EvalResult, EvalError};
pub use output::{EvalOutput, ModuleEvalResult};
pub use operators::OperatorRegistry;
pub use methods::MethodRegistry;
pub use user_methods::{UserMethodRegistry, UserMethod};
pub use unary_operators::UnaryOperatorRegistry;
