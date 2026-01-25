//! Test modules for the evaluator.
//!
//! Comprehensive test suites for evaluator components.
//!
//! ## Organization
//!
//! - `function_val_tests.rs` - Type conversion functions (int, float, str, byte)
//! - `operators_tests.rs` - Binary operator evaluation
//! - `methods_tests.rs` - Method dispatch for built-in types
//! - `unary_operators_tests.rs` - Unary operator evaluation
//! - `environment_tests.rs` - Variable scoping and binding
//! - `expr_tests.rs` - Literal and expression evaluation
//! - `control_tests.rs` - Control flow (if/else, loops, patterns)
//! - `call_tests.rs` - Function and method calls

mod function_val_tests;
mod operators_tests;
mod methods_tests;
mod unary_operators_tests;
mod environment_tests;
mod expr_tests;
mod control_tests;
mod call_tests;
