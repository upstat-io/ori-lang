//! Collection and value construction module.
//!
//! This module handles compilation of various collection and value types:
//! - Tuples
//! - Structs and field access
//! - Option/Result wrappers (Some, None, Ok, Err)
//! - Strings
//! - Lists
//! - Maps
//! - Ranges
//! - Indexing

mod indexing;
mod lists;
mod maps;
mod ranges;
mod strings;
mod structs;
mod tuples;
mod wrappers;

// All methods are implemented on Builder via impl blocks in submodules.
// No re-exports needed - the impl blocks extend Builder directly.
