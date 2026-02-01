//! Function compilation module.
//!
//! This module handles all aspects of function compilation including:
//! - Function bodies
//! - Function calls (direct and method)
//! - Lambdas and closures
//! - Function sequences (run, try, match)
//! - Named function expressions (recurse, parallel, etc.)
//! - PHI node construction
//! - Built-in type conversions

pub mod body;
mod builtins;
mod calls;
mod expressions;
mod helpers;
mod lambdas;
mod phi;
mod sequences;

// All methods are implemented on Builder via impl blocks in submodules.
// No re-exports needed - the impl blocks extend Builder directly.
