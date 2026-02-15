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

        let source = self.source_text();

        // Collect unique lines and multiline labels (borrows table in a block)
        let (mut lines_to_render, multiline_data) = {
            let table = self.line_table();
            let mut lines: Vec<(u32, Vec<usize>)> = Vec::new();
            let mut multiline_indices: Vec<usize> = Vec::new();

            for (i, label) in labels.iter().enumerate() {
                let (start_line, _) = table.offset_to_line_col(source, label.span.start);
                let (end_line, _) = table.offset_to_line_col(source, label.span.end);

                if start_line == end_line {
                    if let Some(entry) = lines.iter_mut().find(|(l, _)| *l == start_line) {
                        entry.1.push(i);
                    } else {
                        lines.push((start_line, vec![i]));
                    }
                } else {
                    multiline_indices.push(i);
                }
            }

            let ml_data: Vec<(usize, u32, u32)> = multiline_indices
                .iter()
                .map(|&idx| {
                    let label = labels[idx];
                    let (sl, _) = table.offset_to_line_col(source, label.span.start);
                    let (el, _) = table.offset_to_line_col(source, label.span.end);
                    (idx, sl, el)
                })
                .collect();

            (lines, ml_data)
        };

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

            // Get line text — borrows from source (independent of self)
            let line_text = {
                let table = self.line_table();
                table.line_text(source, *line_num).unwrap_or("")
            };

            // Emit the source line
            self.write_line_gutter(*line_num, gutter_width);
            let _ = writeln!(self.writer, "{line_text}");

            // Collect underline data: column positions and label message refs
            let mut underline_data: Vec<(usize, usize, bool, &str)> = {
                let table = self.line_table();
                let mut data = Vec::new();
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
                        data.push((
                            start_col,
                            underline_len,
                            label.is_primary,
                            label.message.as_str(),
                        ));
                    }
                }
                data
            };

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
        let source = self.source_text();
        let line_count = end_line - start_line + 1;

        // Pre-compute all line texts and underline data (borrows table in block)
        let (first_text, last_text, underline_len, intermediate_texts) = {
            let table = self.line_table();

            let first = table.line_text(source, start_line).unwrap_or("");
            let last = table.line_text(source, end_line).unwrap_or("");

            // Compute underline data for last line
            let last_line_start = table.line_start_offset(end_line).unwrap_or(0);
            let span_end_on_line = label.span.end.saturating_sub(last_line_start);
            let end_col = table.line_text(source, end_line).map_or(1, |t| {
                let clamped = (span_end_on_line as usize).min(t.len());
                t[..clamped].chars().count()
            });

            // Collect intermediate line texts
            let intermediates: Vec<(u32, &str)> = if line_count <= 4 {
                ((start_line + 1)..end_line)
                    .map(|line| (line, table.line_text(source, line).unwrap_or("")))
                    .collect()
            } else {
                let second = start_line + 1;
                vec![(second, table.line_text(source, second).unwrap_or(""))]
            };

            (first, last, end_col.max(1), intermediates)
        };

        let (pipe_char, caret, color) = if label.is_primary {
            ("/", "^", colors::ERROR)
        } else {
            ("/", "-", colors::SECONDARY)
        };
        let message = &label.message;

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
                self.write_primary(message);
            } else {
                self.write_secondary(message);
            }
        }
        let _ = writeln!(self.writer);
    }

    /// Emit a cross-file label with its own source snippet.
    fn emit_cross_file_snippet(&mut self, label: &Label, gutter_width: usize) {
        let Some(ref src_info) = label.source_info else {
            return;
        };

        // Build a temporary line table for the cross-file source.
        // Cross-file sources are owned by SourceInfo, so we borrow from there.
        let cross_table = LineOffsetTable::build(&src_info.content);
        let (start_line, start_col) =
            cross_table.offset_to_line_col(&src_info.content, label.span.start);
        let (end_line, _) = cross_table.offset_to_line_col(&src_info.content, label.span.end);

        let line_text = cross_table
            .line_text(&src_info.content, start_line)
            .unwrap_or("");
        let cross_gutter_width = digit_count(end_line.max(start_line));
        let path = &src_info.path;
        let message = &label.message;
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
            message,
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
