//! Execution modules for the Ori interpreter.
//!
//! This module contains evaluation logic organized by category:
//!
//! - `expr`: Expression evaluation (literals, binary/unary ops, variables)
//! - `call`: Function call evaluation
//! - `control`: Control flow (if/else, match, loops)
//! - `pattern`: Pattern evaluation (run, try)
//!
//! These modules provide helper functions that the `Interpreter` delegates to.

pub mod expr;
pub mod call;
pub mod control;
pub mod pattern;
