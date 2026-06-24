//! Rich source-snippet rendering for [`TerminalEmitter`] — same-file labels
//! grouped by line, multi-line spans, and cross-file `::: path` snippets.

use std::io::Write;

use crate::span_utils::LineOffsetTable;
use crate::{Diagnostic, Label};

use super::{colors, digit_count, label_columns_on_line, TerminalEmitter};

impl<W: Write> TerminalEmitter<'_, W> {
    /// Emit labels with rich source snippets.
    ///
    /// Groups labels by source (same-file vs cross-file), renders each with
    /// source lines and underline annotations.
    pub(super) fn emit_labels_with_snippets(&mut self, diagnostic: &Diagnostic) {
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
}
