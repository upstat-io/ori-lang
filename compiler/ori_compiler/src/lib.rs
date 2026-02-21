//! Portable Ori compiler driver.
//!
//! Provides a Salsa-free, IO-free compilation pipeline suitable for embedding
//! in WASM, testing harnesses, and other contexts that don't need incremental
//! compilation or filesystem access.
//!
//! # Usage
//!
//! ```ignore
//! use ori_compiler::{compile_and_run, CompileConfig};
//!
//! let output = compile_and_run("@main () -> int = 42", &CompileConfig::default());
//! assert!(output.success);
//! assert_eq!(output.output, "42");
//! ```
//!
//! # Architecture
//!
//! This crate sits between the core compiler crates and the CLI/WASM consumers:
//!
//! ```text
//! ori_ir, ori_lexer, ori_parse, ori_types, ori_canon, ori_eval, ori_fmt
//!                          ↓
//!                    ori_compiler  ← this crate
//!                     /       \
//!                 oric          playground-wasm
//! ```

mod output;
mod pipeline;
mod setup;

pub use output::{CompileOutput, ErrorPhase, FormatOutput};
pub use pipeline::{compile_and_run, format_source, CompileConfig};
pub use setup::setup_module;

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use ori_diagnostic::Diagnostic;

/// Render diagnostics to a string with source context.
///
/// Uses `TerminalEmitter` to produce human-readable output with line numbers,
/// `^` underlines, and error messages. Suitable for embedding in WASM output
/// or test assertions.
pub fn render_diagnostics(
    source: &str,
    file_path: &str,
    diagnostics: &[Diagnostic],
    color: ColorMode,
) -> String {
    let mut buf = Vec::new();
    {
        let mut emitter = TerminalEmitter::with_color_mode(&mut buf, color, false)
            .with_source(source)
            .with_file_path(file_path);
        emitter.emit_all(diagnostics);
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
mod tests;
