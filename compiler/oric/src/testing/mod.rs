//! Testing utilities for the Ori compiler.
//!
//! This module provides test infrastructure for testing the compiler itself:
//!
//! - **harness**: Expression evaluation helpers and assertion utilities
//! - **mocks**: Mock implementations of registries for controlled testing
//!
//! # Usage
//!
//! ```ignore
//! use oric::testing::harness::{eval_source, assert_eval_int};
//!
//! // Test basic arithmetic
//! assert_eval_int("1 + 2", 3);
//!
//! // Test a full program
//! let result = eval_source("@main () -> int = 42");
//! assert_eq!(result.unwrap(), Value::int(42));
//! ```

pub mod harness;
pub mod mocks;

pub use harness::{
    assert_eval_bool, assert_eval_error, assert_eval_float, assert_eval_int, assert_eval_str,
    assert_parse_error, assert_type_error, eval_source, parse_source, type_check_source,
};

pub use mocks::{
    is_bool, is_err, is_float, is_int, is_none, is_ok_with, is_some_with, is_str, test_bool,
    test_char, test_err, test_float, test_int, test_list, test_none, test_ok, test_some, test_str,
    test_tuple, test_void,
};
