//! Portable Ori compiler driver.
//!
//! Provides a Salsa-free, IO-free compilation pipeline suitable for embedding
//! in WASM, testing harnesses, and other contexts that don't need incremental
//! compilation or filesystem access.
//!
//! # Usage
//!
//! ```
//! use ori_compiler::{compile_and_run, CompileConfig};
//!
//! let output = compile_and_run("@main () -> int = 42;", &CompileConfig::default());
//! assert!(output.success);
//! assert_eq!(output.output, "42");
//! ```
//!
//! # Architecture
//!
//! This crate sits above the core compiler crates and is driven by embedder
//! hosts (WASM playgrounds, in-process embeddings):
//!
//! ```text
//! ori_ir, ori_lexer, ori_parse, ori_types, ori_canon, ori_eval, ori_fmt
//!                          ↓
//!                    ori_compiler  ← this crate
//!                          ↓
//!                  WASM / embedder hosts
//! ```

mod diagnostics;
mod output;
mod pipeline;
mod setup;

pub use diagnostics::render_diagnostics;
pub use output::{CompileOutput, ErrorPhase, FormatOutput};
pub use pipeline::{compile_and_run, format_source, CompileConfig};
pub use setup::setup_module;

#[cfg(test)]
mod tests;
