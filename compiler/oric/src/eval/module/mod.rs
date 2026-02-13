//! Module loading and import registration for the evaluator.
//!
//! This module handles:
//! - Building `FunctionValue`s from imported module ASTs
//! - Registering imported items into the evaluator's `Environment`
//! - Managing module function captures and visibility
//!
//! Path resolution (finding files on disk) lives in [`crate::imports`].

pub(crate) mod import;
