//! Terminal Emitter
//!
//! Human-readable diagnostic output with optional ANSI color support.
//! When source text is provided, renders Rust-style source snippets with
//! underlines and labeled spans. Falls back to byte-offset output otherwise.

use std::io::{self, Write};

use crate::span_utils::LineOffsetTable;
use crate::{Diagnostic, Label, Severity};

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

/// Extract the text of a source line using a table (returns owned to avoid borrow issues).
fn extract_line(table: &LineOffsetTable, source: &str, line: u32) -> String {
    table.line_text(source, line).unwrap_or("").to_string()
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
/// When source text is provided via `with_source()`, renders rich Rust-style
/// snippets with source lines, underlines, and labeled spans. Without source
/// text, falls back to byte-offset output for backward compatibility.
pub struct TerminalEmitter<W: Write> {
    writer: W,
    colors: bool,
    /// Source text for rendering snippets.
    source: Option<String>,
    /// File path displayed in `-->` location headers.
    file_path: Option<String>,
    /// Pre-computed line offset table for O(log L) lookups.
    line_table: Option<LineOffsetTable>,
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
    pub fn stdout(mode: ColorMode, is_tty: bool) -> TerminalEmitter<io::Stdout> {
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
    pub fn stderr(mode: ColorMode, is_tty: bool) -> TerminalEmitter<io::Stderr> {
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
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        let source = source.into();
        self.line_table = Some(LineOffsetTable::build(&source));
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

    /// Get source and line table references (panics if `has_source()` is false).
    ///
    /// Callers must ensure `has_source()` before calling.
    #[expect(
        clippy::expect_used,
        reason = "invariant: only called after has_source() check"
    )]
    fn source_ctx(&self) -> (&str, &LineOffsetTable) {
        let source = self
            .source
            .as_deref()
            .expect("source_ctx called without source");
        let table = self
            .line_table
            .as_ref()
            .expect("source_ctx called without line_table");
        (source, table)
    }

    // Low-level write helpers

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

    /// Write the gutter separator: `{padding} |`
    fn write_gutter(&mut self, gutter_width: usize) {
        let padding = " ".repeat(gutter_width + 1);
        self.write_secondary(&format!("{padding}|"));
    }

    /// Write a line number in the gutter: `{line_num} |`
    fn write_line_gutter(&mut self, line_num: u32, gutter_width: usize) {
        let formatted = format!("{line_num:>gutter_width$} | ");
        self.write_secondary(&formatted);
    }

    /// Write an underline with optional label message.
    fn write_underline(
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

    // Snippet rendering

    /// Emit labels with rich source snippets.
    ///
    /// Groups labels by source (same-file vs cross-file), renders each with
    /// source lines and underline annotations.
    fn emit_labels_with_snippets(&mut self, diagnostic: &Diagnostic) {
        // Separate same-file and cross-file labels
        let mut same_file_labels: Vec<&Label> = Vec::new();
        let mut cross_file_labels: Vec<&Label> = Vec::new();

        for label in &diagnostic.labels {
            if label.is_cross_file() {
                cross_file_labels.push(label);
            } else {
                same_file_labels.push(label);
            }
        }

        // Sort same-file labels by span start for deterministic output
        same_file_labels.sort_by_key(|l| l.span.start);

        // Pre-extract table and source refs for column computation
        let (source, table) = self.source_ctx();

        // Find maximum line number for gutter width calculation
        let max_line = same_file_labels
            .iter()
            .map(|l| table.offset_to_line_col(source, l.span.end).0)
            .max()
            .unwrap_or(1);
        let gutter_width = digit_count(max_line);

        // Emit location header from the first primary label
        let header_offset = same_file_labels
            .iter()
            .find(|l| l.is_primary)
            .or(same_file_labels.first())
            .map(|l| l.span.start);

        if let Some(offset) = header_offset {
            let (line, col) = table.offset_to_line_col(source, offset);
            let file = self.file_path.as_deref().unwrap_or("<unknown>").to_string();
            let padding = " ".repeat(gutter_width);
            self.write_secondary(&format!("{padding}-->"));
            let _ = writeln!(self.writer, " {file}:{line}:{col}");
        }

        // Group labels by line and render
        self.emit_same_file_labels(&same_file_labels, gutter_width);

        // Render cross-file labels
        for label in &cross_file_labels {
            self.emit_cross_file_snippet(label, gutter_width);
        }
    }

    /// Render same-file labels grouped by line.
    fn emit_same_file_labels(&mut self, labels: &[&Label], gutter_width: usize) {
        if labels.is_empty() {
            return;
        }

        let (source, table) = self.source_ctx();

        // Collect unique lines that need rendering, in order
        // Also collect multi-line labels separately
        let mut lines_to_render: Vec<(u32, Vec<usize>)> = Vec::new(); // (line, label indices)
        let mut multiline_indices: Vec<usize> = Vec::new();

        for (i, label) in labels.iter().enumerate() {
            let (start_line, _) = table.offset_to_line_col(source, label.span.start);
            let (end_line, _) = table.offset_to_line_col(source, label.span.end);

            if start_line == end_line {
                if let Some(entry) = lines_to_render.iter_mut().find(|(l, _)| *l == start_line) {
                    entry.1.push(i);
                } else {
                    lines_to_render.push((start_line, vec![i]));
                }
            } else {
                multiline_indices.push(i);
            }
        }

        // Pre-compute multiline line ranges before rendering (avoids borrow conflict)
        let multiline_data: Vec<(usize, u32, u32)> = multiline_indices
            .iter()
            .map(|&idx| {
                let label = labels[idx];
                let (start_line, _) = table.offset_to_line_col(source, label.span.start);
                let (end_line, _) = table.offset_to_line_col(source, label.span.end);
                (idx, start_line, end_line)
            })
            .collect();

        // Render multi-line labels first (they emit their own gutter lines)
        for (idx, start_line, end_line) in &multiline_data {
            self.emit_multiline_snippet(labels[*idx], *start_line, *end_line, gutter_width);
        }

        // Sort by line number
        lines_to_render.sort_by_key(|(line, _)| *line);

        // Track whether we need a leading empty gutter line
        let mut prev_line: Option<u32> = None;

        for (line_num, label_indices) in &lines_to_render {
            // Add blank gutter line between non-consecutive lines or at start
            if prev_line.is_none() || prev_line.is_some_and(|p| p + 1 < *line_num) {
                self.write_gutter(gutter_width);
                let _ = writeln!(self.writer);
            }

            // Extract line text as owned string (avoids borrow conflict)
            let (source, table) = self.source_ctx();
            let line_text = extract_line(table, source, *line_num);

            // Emit the source line
            self.write_line_gutter(*line_num, gutter_width);
            let _ = writeln!(self.writer, "{line_text}");

            // Emit underlines for all labels on this line
            // Collect label data upfront to avoid borrow issues
            let (source, table) = self.source_ctx();
            let mut underline_data: Vec<(usize, usize, bool, String)> = Vec::new();

            for &idx in label_indices {
                let label = labels[idx];
                if let Some((start_col, end_col)) =
                    label_columns_on_line(table, source, label, *line_num)
                {
                    let underline_len = if end_col > start_col {
                        end_col - start_col
                    } else {
                        1
                    };
                    underline_data.push((
                        start_col,
                        underline_len,
                        label.is_primary,
                        label.message.clone(),
                    ));
                }
            }

            // Sort by column position (leftmost first)
            underline_data.sort_by_key(|(col, _, _, _)| *col);

            for (start_col, underline_len, is_primary, message) in &underline_data {
                self.write_underline(
                    gutter_width,
                    *start_col,
                    *underline_len,
                    *is_primary,
                    message,
                );
            }

            prev_line = Some(*line_num);
        }

        // Trailing empty gutter
        if !lines_to_render.is_empty() {
            self.write_gutter(gutter_width);
            let _ = writeln!(self.writer);
        }
    }

    /// Emit a multi-line span snippet.
    ///
    /// For spans crossing multiple lines, shows the first line, an elision
    /// if more than 4 lines, and the last line with the underline marker.
    fn emit_multiline_snippet(
        &mut self,
        label: &Label,
        start_line: u32,
        end_line: u32,
        gutter_width: usize,
    ) {
        let (source, table) = self.source_ctx();
        let line_count = end_line - start_line + 1;

        // Pre-extract all line texts we need
        let first_text = extract_line(table, source, start_line);
        let last_text = extract_line(table, source, end_line);

        // Compute underline data for last line
        let last_line_start = table.line_start_offset(end_line).unwrap_or(0);
        let span_end_on_line = label.span.end.saturating_sub(last_line_start);
        let end_col = table.line_text(source, end_line).map_or(1, |t| {
            let clamped = (span_end_on_line as usize).min(t.len());
            t[..clamped].chars().count()
        });
        let underline_len = end_col.max(1);

        let (pipe_char, caret, color) = if label.is_primary {
            ("/", "^", colors::ERROR)
        } else {
            ("/", "-", colors::SECONDARY)
        };
        let message = label.message.clone();

        // Collect intermediate line texts if needed
        let intermediate_texts: Vec<(u32, String)> = {
            let (source, table) = self.source_ctx();
            if line_count <= 4 {
                ((start_line + 1)..end_line)
                    .map(|line| {
                        let text = extract_line(table, source, line);
                        (line, text)
                    })
                    .collect()
            } else {
                let second = start_line + 1;
                let text = extract_line(table, source, second);
                vec![(second, text)]
            }
        };

        // Now do all the writing (no more borrows of self.source/line_table)

        // Leading empty gutter
        self.write_gutter(gutter_width);
        let _ = writeln!(self.writer);

        // First line with `/` marker
        self.write_line_gutter(start_line, gutter_width);
        if self.colors {
            let _ = write!(self.writer, "{color}{pipe_char}{} ", colors::RESET);
        } else {
            let _ = write!(self.writer, "{pipe_char} ");
        }
        let _ = writeln!(self.writer, "{first_text}");

        if line_count <= 4 {
            for (line, text) in &intermediate_texts {
                self.write_line_gutter(*line, gutter_width);
                if self.colors {
                    let _ = write!(self.writer, "{color}|{} ", colors::RESET);
                } else {
                    let _ = write!(self.writer, "| ");
                }
                let _ = writeln!(self.writer, "{text}");
            }
        } else {
            // Show second line
            if let Some((line, text)) = intermediate_texts.first() {
                self.write_line_gutter(*line, gutter_width);
                if self.colors {
                    let _ = write!(self.writer, "{color}|{} ", colors::RESET);
                } else {
                    let _ = write!(self.writer, "| ");
                }
                let _ = writeln!(self.writer, "{text}");
            }

            // Elision
            let padding = " ".repeat(gutter_width + 1);
            if self.colors {
                let _ = writeln!(self.writer, "{padding}{color}|{} ...", colors::RESET);
            } else {
                let _ = writeln!(self.writer, "{padding}| ...");
            }
        }

        // Last line
        self.write_line_gutter(end_line, gutter_width);
        if self.colors {
            let _ = write!(self.writer, "{color}|{} ", colors::RESET);
        } else {
            let _ = write!(self.writer, "| ");
        }
        let _ = writeln!(self.writer, "{last_text}");

        // Underline on last line
        let padding = " ".repeat(gutter_width + 1);
        let underline = caret.repeat(underline_len);
        if self.colors {
            let _ = write!(
                self.writer,
                "{padding}{color}|{} {underline}",
                colors::RESET
            );
        } else {
            let _ = write!(self.writer, "{padding}| {underline}");
        }

        if !message.is_empty() {
            let _ = write!(self.writer, " ");
            if label.is_primary {
                self.write_primary(&message);
            } else {
                self.write_secondary(&message);
            }
        }
        let _ = writeln!(self.writer);
    }

    /// Emit a cross-file label with its own source snippet.
    fn emit_cross_file_snippet(&mut self, label: &Label, gutter_width: usize) {
        let Some(ref src_info) = label.source_info else {
            return;
        };

        // Build a temporary line table for the cross-file source
        let cross_table = LineOffsetTable::build(&src_info.content);
        let (start_line, start_col) =
            cross_table.offset_to_line_col(&src_info.content, label.span.start);
        let (end_line, _) = cross_table.offset_to_line_col(&src_info.content, label.span.end);

        // Pre-extract data before writing
        let line_text = extract_line(&cross_table, &src_info.content, start_line);
        let cross_gutter_width = digit_count(end_line.max(start_line));
        let path = src_info.path.clone();
        let message = label.message.clone();
        let is_primary = label.is_primary;

        // Compute underline columns
        let (start_col_chars, underline_len) = {
            let line_start = cross_table.line_start_offset(start_line).unwrap_or(0);
            let span_start_on_line = label.span.start.saturating_sub(line_start) as usize;
            let line_len = u32::try_from(line_text.len()).unwrap_or(u32::MAX);
            let line_end_byte = line_start.saturating_add(line_len);
            let span_end_on_line =
                label.span.end.min(line_end_byte).saturating_sub(line_start) as usize;

            let sc = line_text[..span_start_on_line.min(line_text.len())]
                .chars()
                .count();
            let ec = line_text[..span_end_on_line.min(line_text.len())]
                .chars()
                .count();
            let len = if ec > sc { ec - sc } else { 1 };
            (sc, len)
        };

        // ::: path:line:col header
        let padding = " ".repeat(gutter_width);
        self.write_secondary(&format!("{padding}:::"));
        let _ = writeln!(self.writer, " {path}:{start_line}:{start_col}");

        // Empty gutter
        self.write_gutter(cross_gutter_width);
        let _ = writeln!(self.writer);

        // Source line
        self.write_line_gutter(start_line, cross_gutter_width);
        let _ = writeln!(self.writer, "{line_text}");

        // Underline
        self.write_underline(
            cross_gutter_width,
            start_col_chars,
            underline_len,
            is_primary,
            &message,
        );
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

impl<W: Write> DiagnosticEmitter for TerminalEmitter<W> {
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

    // Fallback (no source) tests

    #[test]
    fn test_terminal_emitter_no_color() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

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
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, true);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("\x1b["));
        assert!(text.contains("E2001"));
    }

    #[test]
    fn test_emit_all() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

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
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

        emitter.emit_summary(2, 1);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("2 previous errors"));
        assert!(text.contains("1 warning"));
    }

    #[test]
    fn test_emit_summary_single_error() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

        emitter.emit_summary(1, 0);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("previous error"));
        assert!(!text.contains("errors"));
    }

    #[test]
    fn test_emit_summary_warnings_only() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);

        emitter.emit_summary(0, 3);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("3 warnings"));
    }

    // ColorMode tests

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
        assert!(text.contains("\x1b["));
    }

    #[test]
    fn test_with_color_mode_never() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, true);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(!text.contains("\x1b["));
    }

    #[test]
    fn test_with_color_mode_auto_tty() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Auto, true);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("\x1b["));
    }

    #[test]
    fn test_with_color_mode_auto_no_tty() {
        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Auto, false);

        emitter.emit(&sample_diagnostic());
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(!text.contains("\x1b["));
    }

    // Cross-file label tests (fallback)

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
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains(":::"), "Expected ::: marker, got:\n{text}");
        assert!(
            text.contains("src/lib.ori"),
            "Expected file path, got:\n{text}"
        );
        assert!(
            text.contains("return type defined here"),
            "Expected label message, got:\n{text}"
        );
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
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, true);
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains(":::"));
        assert!(text.contains("src/lib.ori"));
        assert!(text.contains("\x1b[1m")); // Bold ANSI code
    }

    // Snippet rendering tests

    #[test]
    fn test_snippet_single_line() {
        // Line 1: "let x = 42\n"      (11 bytes: 0..11)
        // Line 2: "let y = \"hello\"\n" (16 bytes: 11..27)
        // Line 3: "let z = x + y"     (13 bytes: 27..40)
        //                  ^^^^^       span 35..40 = "x + y" (col 9..14)
        let source = "let x = 42\nlet y = \"hello\"\nlet z = x + y";
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(35, 40), "expected `int`, found `str`");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("demo.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();

        // Should contain file:line:col header (col 9 = 'x' in "x + y")
        assert!(
            text.contains("--> demo.ori:3:9"),
            "Expected location header, got:\n{text}"
        );
        // Should contain the source line
        assert!(
            text.contains("let z = x + y"),
            "Expected source line, got:\n{text}"
        );
        // Should contain line number
        assert!(text.contains("3 |"), "Expected line number, got:\n{text}");
        // Should contain underline carets
        assert!(text.contains("^^^^^"), "Expected underline, got:\n{text}");
        // Should contain label message
        assert!(
            text.contains("expected `int`, found `str`"),
            "Expected label message, got:\n{text}"
        );
        // Should NOT contain byte offsets
        assert!(
            !text.contains("35..40"),
            "Should not contain byte offsets, got:\n{text}"
        );
    }

    #[test]
    fn test_snippet_point_span() {
        let source = "let x = 42";
        let diag = Diagnostic::error(ErrorCode::E1001)
            .with_message("unexpected")
            .with_label(Span::new(4, 4), "here");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Point span should still render at least one caret
        assert!(
            text.contains('^'),
            "Expected at least one caret, got:\n{text}"
        );
    }

    #[test]
    fn test_snippet_multiple_labels_same_line() {
        let source = "let result = add(x, y)";
        //                               ^  ^  <- two labels on same line
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(17, 18), "this is `str`")
            .with_secondary_label(Span::new(20, 21), "this is `int`");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("this is `str`"),
            "Expected primary label, got:\n{text}"
        );
        assert!(
            text.contains("this is `int`"),
            "Expected secondary label, got:\n{text}"
        );
        // Primary uses ^, secondary uses -
        assert!(text.contains('^'), "Expected ^ for primary, got:\n{text}");
        assert!(text.contains('-'), "Expected - for secondary, got:\n{text}");
    }

    #[test]
    fn test_snippet_multiple_labels_different_lines() {
        let source = "let x: int = 42\nlet y: str = x";
        //            ^^^^^             ^^^^^^^^^^^^^^
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(16, 30), "expected `str`, found `int`")
            .with_secondary_label(Span::new(0, 15), "defined as `int` here");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("1 |") && text.contains("2 |"),
            "Expected both line numbers, got:\n{text}"
        );
    }

    #[test]
    fn test_snippet_cross_file_with_source() {
        use crate::SourceInfo;

        let source = "let x: int = get_name()";
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(13, 23), "expected `int`, found `str`")
            .with_cross_file_secondary_label(
                Span::new(0, 19),
                "return type defined here",
                SourceInfo::new("src/lib.ori", "@get_name () -> str"),
            );

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("src/main.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Should have --> for main file
        assert!(
            text.contains("--> src/main.ori:1:14"),
            "Expected main file location, got:\n{text}"
        );
        // Should have ::: for cross-file
        assert!(
            text.contains("::: src/lib.ori:1:1"),
            "Expected cross-file location, got:\n{text}"
        );
        // Should show cross-file source
        assert!(
            text.contains("@get_name () -> str"),
            "Expected cross-file source line, got:\n{text}"
        );
        assert!(
            text.contains("return type defined here"),
            "Expected cross-file label, got:\n{text}"
        );
    }

    #[test]
    fn test_snippet_unicode_alignment() {
        // Greek letters: each is 2 bytes in UTF-8, but 1 character column
        // Line 1: "let αβ = 42\n"  (14 bytes: l=1, e=1, t=1, ' '=1, α=2, β=2, ' '=1, '='=1, ' '=1, 4=1, 2=1, \n=1)
        // Line 2: "let γ = αβ + \"hello\""
        //   l=1 e=1 t=1 ' '=1 γ=2 ' '=1 '='=1 ' '=1 α=2 β=2 ' '=1 '+'=1 ' '=1 '"'=1 h=1 e=1 l=1 l=1 o=1 '"'=1
        //   Line 2 starts at byte 14
        //   "hello" (with quotes) starts at byte 14 + 16 = 30, ends at 30 + 7 = 37
        let source = "let αβ = 42\nlet γ = αβ + \"hello\"";
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(30, 37), "expected `int`");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Should contain the source line with unicode
        assert!(
            text.contains("let γ = αβ + \"hello\""),
            "Expected unicode source line, got:\n{text}"
        );
        // Underline should be 7 chars wide (for "hello" including quotes)
        assert!(text.contains("^^^^^^^"), "Expected 7 carets, got:\n{text}");
    }

    #[test]
    fn test_snippet_gutter_width_two_digits() {
        // Create source with 10+ lines so gutter needs 2 digits
        let lines: Vec<String> = (1..=12).map(|i| format!("let x{i} = {i}")).collect();
        let source = lines.join("\n");
        // Error on line 12
        let line12_start = source.rfind("let x12").unwrap() as u32;
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("error on line 12")
            .with_label(Span::new(line12_start, line12_start + 7), "here");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(&source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        // Line 12 should be right-aligned with 2-digit gutter
        assert!(
            text.contains("12 |"),
            "Expected 2-digit line number, got:\n{text}"
        );
    }

    #[test]
    fn test_snippet_with_colors() {
        let source = "let x = 42\nlet y = x + \"hello\"";
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(12, 19), "expected `int`");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Always, false)
            .with_source(source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("\x1b["),
            "Expected ANSI color codes, got:\n{text}"
        );
        assert!(text.contains("expected `int`"));
    }

    #[test]
    fn test_snippet_no_colors() {
        let source = "let x = 42\nlet y = x + \"hello\"";
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(12, 19), "expected `int`");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(
            !text.contains("\x1b["),
            "Should not have ANSI codes, got:\n{text}"
        );
    }

    #[test]
    fn test_fallback_without_source() {
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(10, 15), "expected `int`");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false);
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("10..15"),
            "Expected byte offset fallback, got:\n{text}"
        );
        assert!(
            !text.contains(" | "),
            "Should not have gutter in fallback, got:\n{text}"
        );
    }

    #[test]
    fn test_snippet_notes_and_suggestions() {
        let source = "let x: int = \"hello\"";
        let diag = Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch")
            .with_label(Span::new(13, 20), "expected `int`, found `str`")
            .with_note("int and str are incompatible")
            .with_suggestion("use `int()` to convert");

        let mut output = Vec::new();
        let mut emitter = TerminalEmitter::with_color_mode(&mut output, ColorMode::Never, false)
            .with_source(source)
            .with_file_path("test.ori");
        emitter.emit(&diag);
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(
            text.contains("= note: int and str are incompatible"),
            "Expected note, got:\n{text}"
        );
        assert!(
            text.contains("= help: use `int()` to convert"),
            "Expected suggestion, got:\n{text}"
        );
    }

    // digit_count tests

    #[test]
    fn test_digit_count() {
        assert_eq!(digit_count(0), 1);
        assert_eq!(digit_count(1), 1);
        assert_eq!(digit_count(9), 1);
        assert_eq!(digit_count(10), 2);
        assert_eq!(digit_count(99), 2);
        assert_eq!(digit_count(100), 3);
        assert_eq!(digit_count(999), 3);
        assert_eq!(digit_count(1000), 4);
    }
}
