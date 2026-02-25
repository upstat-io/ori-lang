//! AOT Test Modules
//!
//! Organized by test category following patterns from:
//! - Rust: `tests/run-make/` integration tests
//! - Zig: `test/link/` and `test/standalone/` tests

pub mod arc;
pub mod cli;
pub mod codegen;
pub mod collections_ext;
pub mod conversions;
pub mod cross;
pub mod depth;
pub mod derives;
pub mod error_handling;
pub mod for_loops;
pub mod formattable;
pub mod higher_order;
pub mod iterators;
pub mod linking;
pub mod lto;
pub mod memory_stress;
pub mod mutations;
pub mod operators;
pub mod panic;
pub mod patterns;
pub mod recursion;
pub mod scoping;
pub mod spec;
pub mod stress;
pub mod strings;
pub mod structs;
pub mod traits;
pub mod tuples;
pub mod wasm;

// Re-export test utilities
pub mod util;
