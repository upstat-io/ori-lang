//! Terminal Emitter
//!
//! Human-readable diagnostic output with optional ANSI color support.

use super::{atty_check, DiagnosticEmitter};
use crate::{Diagnostic, Severity};
use std::io::{self, Write};

/// Terminal emitter with optional color support.
pub struct TerminalEmitter<W: Write> {
    writer: W,
    colors: bool,
}

impl<W: Write> TerminalEmitter<W> {
    /// Create a new terminal emitter.
    pub fn new(writer: W, colors: bool) -> Self {
        TerminalEmitter { writer, colors }
    }

    /// Create a terminal emitter for stdout with auto-detected color support.
    pub fn stdout() -> TerminalEmitter<io::Stdout> {
        TerminalEmitter {
            writer: io::stdout(),
            colors: atty_check(),
        }
    }

    /// Create a terminal emitter for stderr with auto-detected color support.
    pub fn stderr() -> TerminalEmitter<io::Stderr> {
        TerminalEmitter {
            writer: io::stderr(),
            colors: atty_check(),
        }
    }

    fn write_severity(&mut self, severity: Severity) {
        if self.colors {
            let color = match severity {
                Severity::Error => "\x1b[1;31m",   // Bold red
                Severity::Warning => "\x1b[1;33m", // Bold yellow
                Severity::Note => "\x1b[1;36m",    // Bold cyan
                Severity::Help => "\x1b[1;32m",    // Bold green
            };
            let _ = write!(self.writer, "{}{}\x1b[0m", color, severity);
        } else {
            let _ = write!(self.writer, "{}", severity);
        }
    }

    fn write_code(&mut self, code: &str) {
        if self.colors {
            let _ = write!(self.writer, "\x1b[1m[{}]\x1b[0m", code);
        } else {
            let _ = write!(self.writer, "[{}]", code);
        }
    }

    fn write_primary(&mut self, text: &str) {
        if self.colors {
            let _ = write!(self.writer, "\x1b[1;31m{}\x1b[0m", text);
        } else {
            let _ = write!(self.writer, "{}", text);
        }
    }

    fn write_secondary(&mut self, text: &str) {
        if self.colors {
            let _ = write!(self.writer, "\x1b[1;34m{}\x1b[0m", text);
        } else {
            let _ = write!(self.writer, "{}", text);
        }
    }
}

impl<W: Write> DiagnosticEmitter for TerminalEmitter<W> {
    fn emit(&mut self, diagnostic: &Diagnostic) {
        // Header: severity[CODE]: message
        self.write_severity(diagnostic.severity);
        self.write_code(diagnostic.code.as_str());
        let _ = writeln!(self.writer, ": {}", diagnostic.message);

        // Labels
        for label in &diagnostic.labels {
            let marker = if label.is_primary { "-->" } else { "   " };
            let _ = write!(self.writer, "  {} {:?}: ", marker, label.span);
            if label.is_primary {
                self.write_primary(&label.message);
            } else {
                self.write_secondary(&label.message);
            }
            let _ = writeln!(self.writer);
        }

        // Notes
        for note in &diagnostic.notes {
            let _ = write!(self.writer, "  = ");
            if self.colors {
                let _ = write!(self.writer, "\x1b[1mnote\x1b[0m");
            } else {
                let _ = write!(self.writer, "note");
            }
            let _ = writeln!(self.writer, ": {}", note);
        }

        // Suggestions
        for suggestion in &diagnostic.suggestions {
            let _ = write!(self.writer, "  = ");
            if self.colors {
                let _ = write!(self.writer, "\x1b[1;32mhelp\x1b[0m");
            } else {
                let _ = write!(self.writer, "help");
            }
            let _ = writeln!(self.writer, ": {}", suggestion);
        }

        let _ = writeln!(self.writer);
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
    }

    fn emit_summary(&mut self, error_count: usize, warning_count: usize) {
        if error_count > 0 || warning_count > 0 {
            if self.colors {
                if error_count > 0 {
                    let _ = write!(
                        self.writer,
                        "\x1b[1;31merror\x1b[0m: aborting due to "
                    );
                    if error_count == 1 {
                        let _ = write!(self.writer, "previous error");
                    } else {
                        let _ = write!(self.writer, "{} previous errors", error_count);
                    }
                    if warning_count > 0 {
                        let _ = write!(
                            self.writer,
                            "; {} warning{} emitted",
                            warning_count,
                            if warning_count == 1 { "" } else { "s" }
                        );
                    }
                    let _ = writeln!(self.writer);
                } else if warning_count > 0 {
                    let _ = writeln!(
                        self.writer,
                        "\x1b[1;33mwarning\x1b[0m: {} warning{} emitted",
                        warning_count,
                        if warning_count == 1 { "" } else { "s" }
                    );
                }
            } else {
                if error_count > 0 {
                    let _ = write!(self.writer, "error: aborting due to ");
                    if error_count == 1 {
                        let _ = write!(self.writer, "previous error");
                    } else {
                        let _ = write!(self.writer, "{} previous errors", error_count);
                    }
                    if warning_count > 0 {
                        let _ = write!(
                            self.writer,
                            "; {} warning{} emitted",
                            warning_count,
                            if warning_count == 1 { "" } else { "s" }
                        );
                    }
                    let _ = writeln!(self.writer);
                } else if warning_count > 0 {
                    let _ = writeln!(
                        self.writer,
                        "warning: {} warning{} emitted",
                        warning_count,
                        if warning_count == 1 { "" } else { "s" }
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCode;
    use sigil_ir::Span;

    fn sample_diagnostic() -> Diagnostic {
        Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch: expected `int`, found `str`")
            .with_label(Span::new(10, 15), "expected `int`")
            .with_secondary_label(Span::new(0, 5), "defined here")
            .with_note("int and str are incompatible")
            .with_suggestion("use `int(x)` to convert")
    }

    #[test]
    fn test_terminal_emitter_no_color() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, false);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("error"));
        assert!(text.contains("[E2001]"));
        assert!(text.contains("type mismatch"));
        assert!(text.contains("expected `int`"));
        assert!(text.contains("note:"));
        assert!(text.contains("help:"));
    }

    #[test]
    fn test_terminal_emitter_with_color() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, true);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Check for ANSI escape codes
        assert!(text.contains("\x1b["));
        assert!(text.contains("E2001"));
    }

    #[test]
    fn test_emit_all() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, false);

        let diagnostics = vec![
            Diagnostic::error(ErrorCode::E1001).with_message("error 1"),
            Diagnostic::error(ErrorCode::E2001).with_message("error 2"),
        ];

        emitter.emit_all(&diagnostics);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("error 1"));
        assert!(text.contains("error 2"));
    }

    #[test]
    fn test_emit_summary_errors() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, false);

        emitter.emit_summary(2, 1);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("2 previous errors"));
        assert!(text.contains("1 warning"));
    }

    #[test]
    fn test_emit_summary_single_error() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, false);

        emitter.emit_summary(1, 0);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("previous error"));
        assert!(!text.contains("errors"));
    }

    #[test]
    fn test_emit_summary_warnings_only() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, false);

        emitter.emit_summary(0, 3);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("3 warnings"));
    }
}
