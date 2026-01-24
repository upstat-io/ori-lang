//! Execution modules for the Sigil evaluator.
//!
//! This module contains extracted evaluation logic organized by category:
//!
//! - `expr`: Expression evaluation (literals, binary/unary ops, variables)
//! - `call`: Function and method call evaluation
//! - `control`: Control flow (if/else, match, loops)
//! - `pattern`: Pattern evaluation (run, try, map, filter, fold, etc.)
//!
//! These modules provide helper functions that the main `Evaluator` delegates to,
//! allowing the evaluator.rs file to focus on coordination rather than
//! implementation details.

pub mod expr;
pub mod call;
pub mod control;
pub mod pattern;
