//! Diagnostic Emitters
//!
//! Provides different output formats for diagnostics:
//! - Terminal: Colored, human-readable output
//! - JSON: Machine-readable output for tooling
//! - SARIF: Static Analysis Results Interchange Format for CI/CD integration
//!
//! Each emitter implements the `DiagnosticEmitter` trait and can be
//! configured for different use cases.

mod json;
mod sarif;
mod terminal;

pub use json::JsonEmitter;
pub use sarif::SarifEmitter;
pub use terminal::TerminalEmitter;

use std::fmt::Write;

use crate::Diagnostic;

/// Returns a trailing comma for JSON/SARIF list serialization.
///
/// Returns `","` when `index` is not the last element, `""` otherwise.
pub(crate) fn trailing_comma(index: usize, total: usize) -> &'static str {
    if index + 1 < total {
        ","
    } else {
        ""
    }
}

/// Trait for emitting diagnostics in various formats.
pub trait DiagnosticEmitter {
    /// Emit a single diagnostic.
    fn emit(&mut self, diagnostic: &Diagnostic);

    /// Emit multiple diagnostics.
    fn emit_all(&mut self, diagnostics: &[Diagnostic]) {
        for diag in diagnostics {
            self.emit(diag);
        }
    }

    /// Flush any buffered output.
    fn flush(&mut self);

    /// Emit a summary of errors/warnings.
    fn emit_summary(&mut self, error_count: usize, warning_count: usize);
}

/// Escape a string for JSON output.
pub(crate) fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(result, "\\u{:04x}", c as u32);
            }
            c => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json() {
        assert_eq!(escape_json("hello"), "hello");
        assert_eq!(escape_json("\"quoted\""), "\\\"quoted\\\"");
        assert_eq!(escape_json("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_json("path\\file"), "path\\\\file");
        assert_eq!(escape_json("tab\there"), "tab\\there");
    }
}
