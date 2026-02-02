//! AOT Test Modules
//!
//! Organized by test category following patterns from:
//! - Rust: `tests/run-make/` integration tests
//! - Zig: `test/link/` and `test/standalone/` tests

pub mod cli;
pub mod codegen;
pub mod cross;
pub mod linking;
pub mod lto;
pub mod spec;
pub mod wasm;

// Re-export test utilities
pub mod util;
