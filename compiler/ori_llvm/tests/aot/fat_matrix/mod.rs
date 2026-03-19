//! Fat Pointer Test Matrix
//!
//! Systematic combinatorial testing of {type categories} x {language features}.
//! Each sub-module tests one feature dimension across all applicable type categories.
//! See `plans/fat-pointer-hardening/section-04-test-matrix.md` for the full matrix.

pub mod f01_let_binding;
pub mod f02_function_param;
pub mod f03_function_return;
pub mod f04_closure_capture;
pub mod f07_branching;
pub mod f08_for_loop;
pub mod f09_loop_accumulation;
pub mod f14_list_element;
pub mod f18_multiple_values;
pub mod f19_break_continue;
