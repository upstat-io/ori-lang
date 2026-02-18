//! AOT Test Modules
//!
//! Organized by test category following patterns from:
//! - Rust: `tests/run-make/` integration tests
//! - Zig: `test/link/` and `test/standalone/` tests

pub mod cli;
pub mod codegen;
pub mod cross;
pub mod derives;
pub mod for_loops;
pub mod linking;
pub mod lto;
pub mod spec;
pub mod traits;
pub mod wasm;

// Re-export test utilities
pub mod util;
