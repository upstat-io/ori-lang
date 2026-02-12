//! Execution modules for the Ori interpreter.
//!
//! This module contains evaluation logic organized by category:
//!
//! - `expr`: Expression evaluation (literals, binary/unary ops, variables)
//! - `call`: Function call evaluation
//! - `control`: Control flow (if/else, match, loops)
//! - `decision_tree`: Compiled decision tree evaluation (Section 03.4)
//!
//! These modules provide helper functions that the `Interpreter` delegates to.

pub mod call;
pub mod control;
pub mod decision_tree;
pub mod expr;
