//! Terminal Emitter
//!
//! Human-readable diagnostic output with optional ANSI color support.
//! When source text is provided, renders source snippets with underlines and
//! labeled spans. Falls back to byte-offset output otherwise.

use std::io::{self, Write};

use crate::span_utils::LineOffsetTable;
use crate::{Diagnostic, Label};

use super::DiagnosticEmitter;

mod snippet;
mod write_helpers;

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

/// Compute the number of decimal digits needed to display a line number.
#[inline]
fn digit_count(mut n: u32) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    while n > 0 {
        count += 1;
        n /= 10;
    }
    count
}

/// Compute (`start_col_chars`, `end_col_chars`) for a label on a given line.
///
/// All values are character-based (not byte-based) for correct unicode alignment.
fn label_columns_on_line(
    table: &LineOffsetTable,
    source: &str,
    label: &Label,
    line_num: u32,
) -> Option<(usize, usize)> {
    let line_start = table.line_start_offset(line_num)?;
    let line_text = table.line_text(source, line_num)?;
    let line_len = u32::try_from(line_text.len()).unwrap_or(u32::MAX);
    let line_end_offset = line_start.saturating_add(line_len);

    let span_start_on_line = (label.span.start.max(line_start) - line_start) as usize;
    let span_end_on_line = (label.span.end.min(line_end_offset) - line_start) as usize;

    let start_col = line_text[..span_start_on_line.min(line_text.len())]
        .chars()
        .count();
    let end_col = line_text[..span_end_on_line.min(line_text.len())]
        .chars()
        .count();

    Some((start_col, end_col))
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

/// Terminal emitter with optional color support and source snippet rendering.
///
/// When source text is provided via `with_source()`, renders rich snippets
/// with source lines, underlines, and labeled spans. Without source
/// text, falls back to byte-offset output for backward compatibility.
///
/// The `'src` lifetime ties to the source text, which is borrowed (not cloned).
/// The emitter is short-lived (created, used, dropped within a single function),
/// so this borrow is always valid.
pub struct TerminalEmitter<'src, W: Write> {
    writer: W,
    colors: bool,
    /// Source text for rendering snippets (borrowed, not cloned).
    source: Option<&'src str>,
    /// File path displayed in `-->` location headers.
    file_path: Option<String>,
    /// Pre-computed line offset table for O(log L) lookups.
    line_table: Option<LineOffsetTable>,
}

impl<'src, W: Write> TerminalEmitter<'src, W> {
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
            source: None,
            file_path: None,
            line_table: None,
        }
    }

    /// Create a terminal emitter for stdout with explicit color mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Color mode selection (`Auto`, `Always`, or `Never`)
    /// * `is_tty` - Whether stdout is a TTY (used for `ColorMode::Auto`)
    pub fn stdout(mode: ColorMode, is_tty: bool) -> TerminalEmitter<'src, io::Stdout> {
        TerminalEmitter {
            writer: io::stdout(),
            colors: mode.should_use_colors(is_tty),
            source: None,
            file_path: None,
            line_table: None,
        }
    }

    /// Create a terminal emitter for stderr with explicit color mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Color mode selection (`Auto`, `Always`, or `Never`)
    /// * `is_tty` - Whether stderr is a TTY (used for `ColorMode::Auto`)
    pub fn stderr(mode: ColorMode, is_tty: bool) -> TerminalEmitter<'src, io::Stderr> {
        TerminalEmitter {
            writer: io::stderr(),
            colors: mode.should_use_colors(is_tty),
            source: None,
            file_path: None,
            line_table: None,
        }
    }

    /// Set the source text for rendering snippets.
    ///
    /// Builds a line offset table for O(log L) lookups where L is the line count.
    /// When source is set, `emit()` renders rich snippets instead of byte offsets.
    ///
    /// The source is borrowed, not cloned — eliminating one full source-file
    /// allocation per compile.
    #[must_use]
    pub fn with_source(mut self, source: &'src str) -> Self {
        self.line_table = Some(LineOffsetTable::build(source));
        self.source = Some(source);
        self
    }

    /// Set the file path displayed in location headers.
    #[must_use]
    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Check if rich snippet rendering is available.
    fn has_source(&self) -> bool {
        self.source.is_some() && self.line_table.is_some()
    }

    /// Get the source text reference.
    ///
    /// Returns `&'src str` — independent of the `&self` borrow. This is the key
    /// to eliminating allocations: the returned reference doesn't prevent calling
    /// `&mut self` methods afterward.
    ///
    /// Callers must ensure `has_source()` before calling.
    #[expect(
        clippy::expect_used,
        reason = "invariant: only called after has_source() check"
    )]
    fn source_text(&self) -> &'src str {
        self.source.expect("source_text called without source")
    }

    /// Get source and line table references (panics if `has_source()` is false).
    ///
    /// Callers must ensure `has_source()` before calling.
    #[expect(
        clippy::expect_used,
        reason = "invariant: only called after has_source() check"
    )]
    fn source_ctx(&self) -> (&'src str, &LineOffsetTable) {
        let source = self.source.expect("source_ctx called without source");
        let table = self
            .line_table
            .as_ref()
            .expect("source_ctx called without line_table");
        (source, table)
    }

    /// Get the line offset table (panics if `has_source()` is false).
    ///
    /// Used in scoped blocks where `source_ctx()` can't be used because the table
    /// borrow must end before `&mut self` calls.
    #[expect(
        clippy::expect_used,
        reason = "invariant: only called after has_source() check"
    )]
    fn line_table(&self) -> &LineOffsetTable {
        self.line_table
            .as_ref()
            .expect("line_table called without source")
    }

    // Fallback (byte-offset) rendering

    /// Emit labels in the legacy byte-offset format (when no source is available).
    fn emit_labels_fallback(&mut self, diagnostic: &Diagnostic) {
        for label in &diagnostic.labels {
            let marker = if label.is_cross_file() {
                ":::"
            } else if label.is_primary {
                "-->"
            } else {
                "   "
            };

            let _ = write!(self.writer, "  {marker} ");

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
                self.write_secondary(&label.message);
            } else if label.is_primary {
                self.write_primary(&label.message);
            } else {
                self.write_secondary(&label.message);
            }
            let _ = writeln!(self.writer);
        }
    }

    /// Emit notes and suggestions (shared between snippet and fallback paths).
    fn emit_notes_and_suggestions(&mut self, diagnostic: &Diagnostic) {
        for note in &diagnostic.notes {
            let _ = write!(self.writer, "  = ");
            if self.colors {
                let _ = write!(self.writer, "{}note{}", colors::BOLD, colors::RESET);
            } else {
                let _ = write!(self.writer, "note");
            }
            let _ = writeln!(self.writer, ": {note}");
        }

        for suggestion in &diagnostic.suggestions {
            let _ = write!(self.writer, "  = ");
            if self.colors {
                let _ = write!(self.writer, "{}help{}", colors::HELP, colors::RESET);
            } else {
                let _ = write!(self.writer, "help");
            }
            let _ = writeln!(self.writer, ": {suggestion}");
        }

        for suggestion in &diagnostic.structured_suggestions {
            let _ = write!(self.writer, "  = ");
            if self.colors {
                let _ = write!(self.writer, "{}help{}", colors::HELP, colors::RESET);
            } else {
                let _ = write!(self.writer, "help");
            }
            let _ = writeln!(self.writer, ": {}", suggestion.message);
        }
    }
}

impl<W: Write> DiagnosticEmitter for TerminalEmitter<'_, W> {
    fn emit(&mut self, diagnostic: &Diagnostic) {
        // Header: severity[CODE]: message
        self.write_severity(diagnostic.severity);
        self.write_code(diagnostic.code.as_str());
        let _ = writeln!(self.writer, ": {}", diagnostic.message);

        // Labels: rich snippets or fallback
        if self.has_source() && !diagnostic.labels.is_empty() {
            self.emit_labels_with_snippets(diagnostic);
        } else {
            self.emit_labels_fallback(diagnostic);
        }

        // Notes and suggestions (same in both paths)
        self.emit_notes_and_suggestions(diagnostic);

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
            self.write_colored("error", colors::ERROR);

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
#[expect(clippy::cast_possible_truncation, reason = "test offsets are small")]
mod tests;
