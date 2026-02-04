//! Terminal Emitter
//!
//! Human-readable diagnostic output with optional ANSI color support.

use std::io::{self, Write};

use crate::{Diagnostic, Severity};

use super::DiagnosticEmitter;

/// ANSI color codes for terminal output.
mod colors {
    pub const ERROR: &str = "\x1b[1;31m"; // Bold red
    pub const WARNING: &str = "\x1b[1;33m"; // Bold yellow
    pub const NOTE: &str = "\x1b[1;36m"; // Bold cyan
    pub const HELP: &str = "\x1b[1;32m"; // Bold green
    pub const BOLD: &str = "\x1b[1m";
    pub const SECONDARY: &str = "\x1b[1;34m"; // Bold blue
    pub const RESET: &str = "\x1b[0m";
}

/// Returns "s" for plural counts, "" for singular.
#[inline]
fn plural_s(count: usize) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

/// Color output mode for terminal emitter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    /// Automatically detect based on terminal capabilities.
    #[default]
    Auto,
    /// Always use colors.
    Always,
    /// Never use colors.
    Never,
}

impl ColorMode {
    /// Resolve to a boolean based on terminal detection.
    ///
    /// For `Auto` mode, `is_tty` determines whether colors should be used.
    /// This parameter is ignored for `Always` and `Never` modes.
    ///
    /// # Arguments
    ///
    /// * `is_tty` - Whether the output is a TTY (from CLI layer detection)
    pub fn should_use_colors(self, is_tty: bool) -> bool {
        match self {
            ColorMode::Auto => is_tty,
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
    }
}

/// Terminal emitter with optional color support.
pub struct TerminalEmitter<W: Write> {
    writer: W,
    colors: bool,
}

impl<W: Write> TerminalEmitter<W> {
    /// Create a new terminal emitter with explicit color mode.
    ///
    /// # Arguments
    ///
    /// * `writer` - The output writer
    /// * `mode` - Color mode selection
    /// * `is_tty` - Whether output is a TTY (used for `ColorMode::Auto`)
    pub fn with_color_mode(writer: W, mode: ColorMode, is_tty: bool) -> Self {
        TerminalEmitter {
            writer,
            colors: mode.should_use_colors(is_tty),
        }
    }

    /// Create a new terminal emitter with a boolean color flag.
    ///
    /// # Deprecation
    ///
    /// Prefer `with_color_mode` for clearer intent. This method exists for
    /// backwards compatibility but may be removed in a future version.
    #[deprecated(since = "0.2.0", note = "use `with_color_mode` instead")]
    pub fn new(writer: W, colors: bool) -> Self {
        TerminalEmitter { writer, colors }
    }

    /// Create a terminal emitter for stdout with explicit color mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Color mode selection (`Auto`, `Always`, or `Never`)
    /// * `is_tty` - Whether stdout is a TTY (used for `ColorMode::Auto`)
    pub fn stdout(mode: ColorMode, is_tty: bool) -> TerminalEmitter<io::Stdout> {
        TerminalEmitter {
            writer: io::stdout(),
            colors: mode.should_use_colors(is_tty),
        }
    }

    /// Create a terminal emitter for stderr with explicit color mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Color mode selection (`Auto`, `Always`, or `Never`)
    /// * `is_tty` - Whether stderr is a TTY (used for `ColorMode::Auto`)
    pub fn stderr(mode: ColorMode, is_tty: bool) -> TerminalEmitter<io::Stderr> {
        TerminalEmitter {
            writer: io::stderr(),
            colors: mode.should_use_colors(is_tty),
        }
    }

    /// Write text with optional ANSI color codes.
    fn write_colored(&mut self, text: &str, color: &str) {
        if self.colors {
            let _ = write!(self.writer, "{color}{text}{}", colors::RESET);
        } else {
            let _ = write!(self.writer, "{text}");
        }
    }

    fn write_severity(&mut self, severity: Severity) {
        if self.colors {
            let color = match severity {
                Severity::Error => colors::ERROR,
                Severity::Warning => colors::WARNING,
                Severity::Note => colors::NOTE,
                Severity::Help => colors::HELP,
            };
            let _ = write!(self.writer, "{color}{severity}{}", colors::RESET);
        } else {
            let _ = write!(self.writer, "{severity}");
        }
    }

    fn write_code(&mut self, code: &str) {
        if self.colors {
            let _ = write!(self.writer, "{}[{code}]{}", colors::BOLD, colors::RESET);
        } else {
            let _ = write!(self.writer, "[{code}]");
        }
    }

    fn write_primary(&mut self, text: &str) {
        self.write_colored(text, colors::ERROR);
    }

    fn write_secondary(&mut self, text: &str) {
        self.write_colored(text, colors::SECONDARY);
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
            // Cross-file labels use ::: notation to indicate a different file
            let marker = if label.is_cross_file() {
                ":::"
            } else if label.is_primary {
                "-->"
            } else {
                "   "
            };

            let _ = write!(self.writer, "  {marker} ");

            // Include file path for cross-file labels
            if let Some(ref src) = label.source_info {
                if self.colors {
                    let _ = write!(self.writer, "{}{}{}", colors::BOLD, src.path, colors::RESET);
                } else {
                    let _ = write!(self.writer, "{}", src.path);
                }
                let _ = write!(self.writer, " ");
            }

            let _ = write!(self.writer, "{:?}: ", label.span);

            if label.is_cross_file() {
                // Cross-file labels use secondary color (blue)
                self.write_secondary(&label.message);
            } else if label.is_primary {
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
                let _ = write!(self.writer, "{}note{}", colors::BOLD, colors::RESET);
            } else {
                let _ = write!(self.writer, "note");
            }
            let _ = writeln!(self.writer, ": {note}");
        }

        // Suggestions
        for suggestion in &diagnostic.suggestions {
            let _ = write!(self.writer, "  = ");
            if self.colors {
                let _ = write!(self.writer, "{}help{}", colors::HELP, colors::RESET);
            } else {
                let _ = write!(self.writer, "help");
            }
            let _ = writeln!(self.writer, ": {suggestion}");
        }

        let _ = writeln!(self.writer);
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
    }

    fn emit_summary(&mut self, error_count: usize, warning_count: usize) {
        if error_count == 0 && warning_count == 0 {
            return;
        }

        if error_count > 0 {
            // Write "error" prefix
            self.write_colored("error", colors::ERROR);

            // Build message
            let error_part = if error_count == 1 {
                "previous error".to_string()
            } else {
                format!("{error_count} previous errors")
            };

            if warning_count > 0 {
                let _ = writeln!(
                    self.writer,
                    ": aborting due to {error_part}; {} warning{} emitted",
                    warning_count,
                    plural_s(warning_count)
                );
            } else {
                let _ = writeln!(self.writer, ": aborting due to {error_part}");
            }
        } else if warning_count > 0 {
            // Write "warning" prefix
            self.write_colored("warning", colors::WARNING);
            let _ = writeln!(
                self.writer,
                ": {} warning{} emitted",
                warning_count,
                plural_s(warning_count)
            );
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#[allow(deprecated)] // Tests exercise deprecated `new` method for backwards compatibility
mod tests {
    use super::*;
    use crate::ErrorCode;
    use ori_ir::Span;

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

    // --- ColorMode Tests ---

    #[test]
    fn test_color_mode_auto_with_tty() {
        assert!(ColorMode::Auto.should_use_colors(true));
    }

    #[test]
    fn test_color_mode_auto_without_tty() {
        assert!(!ColorMode::Auto.should_use_colors(false));
    }

    #[test]
    fn test_color_mode_always_ignores_tty() {
        assert!(ColorMode::Always.should_use_colors(false));
        assert!(ColorMode::Always.should_use_colors(true));
    }

    #[test]
    fn test_color_mode_never_ignores_tty() {
        assert!(!ColorMode::Never.should_use_colors(false));
        assert!(!ColorMode::Never.should_use_colors(true));
    }

    #[test]
    fn test_color_mode_default_is_auto() {
        assert_eq!(ColorMode::default(), ColorMode::Auto);
    }

    #[test]
    fn test_with_color_mode_always() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, false);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Even without TTY, Always mode produces ANSI codes
        assert!(text.contains("\x1b["));
    }

    #[test]
    fn test_with_color_mode_never() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, true);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Even with TTY, Never mode produces no ANSI codes
        assert!(!text.contains("\x1b["));
    }

    #[test]
    fn test_with_color_mode_auto_tty() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Auto, true);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // With TTY, Auto mode produces ANSI codes
        assert!(text.contains("\x1b["));
    }

    #[test]
    fn test_with_color_mode_auto_no_tty() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Auto, false);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Without TTY, Auto mode produces no ANSI codes
        assert!(!text.contains("\x1b["));
    }

    // --- Cross-file Label Tests ---

    #[test]
    fn test_terminal_emitter_cross_file_label() {
        use crate::SourceInfo;

        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(10, 20), "expected `int`, found `str`")
            .with_cross_file_secondary_label(
                Span::new(0, 19),
                "return type defined here",
                SourceInfo::new("src/lib.ori", "@get_name () -> str"),
            );

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, false);
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Should use ::: marker for cross-file labels
        assert!(text.contains(":::"), "Expected ::: marker, got:\n{text}");
        // Should include the file path
        assert!(
            text.contains("src/lib.ori"),
            "Expected file path, got:\n{text}"
        );
        // Should include the label message
        assert!(
            text.contains("return type defined here"),
            "Expected label message, got:\n{text}"
        );
        // Should still have --> for same-file primary
        assert!(text.contains("-->"), "Expected --> marker, got:\n{text}");
    }

    #[test]
    fn test_terminal_emitter_cross_file_with_colors() {
        use crate::SourceInfo;

        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(10, 20), "expected `int`")
            .with_cross_file_secondary_label(
                Span::new(0, 19),
                "defined here",
                SourceInfo::new("src/lib.ori", "@foo () -> str"),
            );

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::new(&mut output, true);
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Cross-file labels should have ::: and file path
        assert!(text.contains(":::"));
        assert!(text.contains("src/lib.ori"));
        // File path should be bold (check for ANSI bold code before path)
        assert!(text.contains("\x1b[1m")); // Bold ANSI code
    }
}
