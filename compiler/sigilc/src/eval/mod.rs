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
//! methods on `Value`. See `value/mod.rs` for details.

mod value;
mod environment;
mod evaluator;
pub mod errors;
pub mod operators;
pub mod methods;
pub mod unary_operators;

pub use value::{Value, FunctionValue, RangeValue, StructValue, StructLayout, Heap, FunctionValFn};
pub use environment::Environment;
pub use evaluator::{Evaluator, EvalResult, EvalError, EvalOutput, ModuleEvalResult};
pub use operators::OperatorRegistry;
pub use methods::MethodRegistry;
pub use unary_operators::UnaryOperatorRegistry;
