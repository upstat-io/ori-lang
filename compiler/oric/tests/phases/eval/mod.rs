//! Interpreter phase tests.
//!
//! Tests for the `ori_eval` and `ori_patterns` crates, validating:
//! - Expression evaluation
//! - Pattern matching
//! - Method dispatch
//! - Value representation
//! - Runtime behavior
//!
//! # Test Organization
//!
//! - `scalar_int` - `ScalarInt` checked arithmetic and bitwise operations
//! - `pattern_errors` - `EvalError` and error factory functions

mod pattern_errors;
mod scalar_int;
