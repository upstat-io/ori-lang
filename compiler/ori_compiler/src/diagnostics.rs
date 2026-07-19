//! Diagnostic rendering for the portable compiler pipeline.
//!
//! Turns a slice of [`Diagnostic`] values into human-readable text with source
//! context, suitable for embedding in WASM output or test assertions.

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
