//! Tree-walking interpreter for Sigil V2.
//!
//! This module provides runtime evaluation of Sigil programs using a
//! tree-walking interpreter optimized for:
//! - Fast scope lookup via stack-based environment
//! - O(1) struct field access via StructLayout
//! - Efficient closure capture
//!
//! ## Design
//!
//! The interpreter uses a `Value` enum to represent runtime values and
//! an `Environment` struct for variable scoping. Unlike some implementations
//! that clone environments for closures, this uses a scope stack with
//! explicit capture for better performance.

mod value;
mod environment;
mod evaluator;

pub use value::{Value, StructValue, StructLayout, FunctionValue, RangeValue};
pub use environment::{Environment, Scope};
pub use evaluator::Evaluator;
