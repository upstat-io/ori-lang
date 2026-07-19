//! Low-level write primitives for [`TerminalEmitter`] — colored text, severity
//! and code headers, gutters, and underline markers.

use std::io::Write;

use crate::Severity;

use super::{colors, TerminalEmitter};

impl<W: Write> TerminalEmitter<'_, W> {
    /// Write text with optional ANSI color codes.
    pub(super) fn write_colored(&mut self, text: &str, color: &str) {
        if self.colors {
            let _ = write!(self.writer, "{color}{text}{}", colors::RESET);
        } else {
            let _ = write!(self.writer, "{text}");
        }
    }

    pub(super) fn write_severity(&mut self, severity: Severity) {
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

    pub(super) fn write_code(&mut self, code: &str) {
        if self.colors {
            let _ = write!(self.writer, "{}[{code}]{}", colors::BOLD, colors::RESET);
        } else {
            let _ = write!(self.writer, "[{code}]");
        }
    }

    pub(super) fn write_primary(&mut self, text: &str) {
        self.write_colored(text, colors::ERROR);
    }

    pub(super) fn write_secondary(&mut self, text: &str) {
        self.write_colored(text, colors::SECONDARY);
    }

    /// Write the gutter separator: `{padding} |`
    pub(super) fn write_gutter(&mut self, gutter_width: usize) {
        let padding = " ".repeat(gutter_width + 1);
        self.write_secondary(&format!("{padding}|"));
    }

    /// Write a line number in the gutter: `{line_num} |`
    pub(super) fn write_line_gutter(&mut self, line_num: u32, gutter_width: usize) {
        let formatted = format!("{line_num:>gutter_width$} | ");
        self.write_secondary(&formatted);
    }

    /// Write an underline with optional label message.
    pub(super) fn write_underline(
        &mut self,
        gutter_width: usize,
        start_col: usize,
        underline_len: usize,
        is_primary: bool,
        message: &str,
    ) {
        let padding = " ".repeat(gutter_width + 1);
        let lead_spaces = " ".repeat(start_col);
        let (caret, color) = if is_primary {
            ("^", colors::ERROR)
        } else {
            ("-", colors::SECONDARY)
        };
        let underline = caret.repeat(underline_len);

        if self.colors {
            let _ = write!(
                self.writer,
                "{}{color}|{} {lead_spaces}",
                colors::SECONDARY,
                colors::RESET
            );
            self.write_colored(&underline, color);
        } else {
            let _ = write!(self.writer, "{padding}| {lead_spaces}{underline}");
        }

        if !message.is_empty() {
            let _ = write!(self.writer, " ");
            if is_primary {
                self.write_primary(message);
            } else {
                self.write_secondary(message);
            }
        }
        let _ = writeln!(self.writer);
    }
}
