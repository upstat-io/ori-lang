//! Span utility functions for diagnostic processing.
//!
//! Provides helpers for computing line and column numbers from spans,
//! used by `DiagnosticQueue` for sorting and deduplication.
//!
//! ## Performance
//!
//! For repeated lookups on the same source, use [`LineOffsetTable`] which
//! pre-computes line offsets for O(log L) lookup instead of O(n) scanning.

use ori_ir::Span;

/// Pre-computed line offset table for efficient line/column lookup.
///
/// Builds a table of byte offsets for each line start, enabling O(log L)
/// binary search lookups instead of O(n) linear scans. Essential when
/// processing multiple diagnostics with multiple labels each.
///
/// # Example
///
/// ```
/// use ori_diagnostic::span_utils::LineOffsetTable;
///
/// let source = "line1\nline2\nline3";
/// let table = LineOffsetTable::build(source);
///
/// assert_eq!(table.offset_to_line_col(source, 0), (1, 1));  // 'l' in line1
/// assert_eq!(table.offset_to_line_col(source, 6), (2, 1));  // 'l' in line2
/// assert_eq!(table.offset_to_line_col(source, 12), (3, 1)); // 'l' in line3
/// ```
#[derive(Clone, Debug, Default)]
pub struct LineOffsetTable {
    /// Byte offset of each line start (0-indexed lines internally).
    /// offsets[0] = 0 (line 1 starts at byte 0)
    /// offsets[1] = byte after first \n (line 2 start)
    /// etc.
    offsets: Vec<u32>,
}

impl LineOffsetTable {
    /// Build a line offset table from source text.
    ///
    /// Scans the source once to find all newlines, O(n) construction
    /// for O(log L) lookups where L is the number of lines.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "source files are limited to u32::MAX bytes"
    )]
    pub fn build(source: &str) -> Self {
        let mut offsets = vec![0u32];
        for (i, byte) in source.as_bytes().iter().enumerate() {
            if *byte == b'\n' {
                // Next line starts at byte after the newline
                offsets.push((i + 1) as u32);
            }
        }
        LineOffsetTable { offsets }
    }

    /// Get 1-based line number from a byte offset using binary search.
    ///
    /// Returns the line number (1-indexed) containing the given byte offset.
    #[inline]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "line count limited by source file size (u32)"
    )]
    pub fn line_from_offset(&self, offset: u32) -> u32 {
        // Binary search for the largest line start <= offset
        let line_idx = match self.offsets.binary_search(&offset) {
            Ok(exact) => exact,                      // Exact match: offset is at line start
            Err(insert) => insert.saturating_sub(1), // Insert point: line is before
        };
        (line_idx as u32) + 1 // Convert to 1-based
    }

    /// Get 1-based (line, column) from a byte offset.
    ///
    /// The column is computed as the number of characters (not bytes)
    /// from the start of the line. Uses binary search for line lookup.
    pub fn offset_to_line_col(&self, source: &str, offset: u32) -> (u32, u32) {
        let line = self.line_from_offset(offset);
        let line_idx = (line - 1) as usize;
        let line_start = self.offsets.get(line_idx).copied().unwrap_or(0) as usize;
        let offset = offset as usize;

        // Column is 1-based, counting characters from line start to offset
        let col_bytes = &source[line_start..offset.min(source.len())];
        let col = u32::try_from(col_bytes.chars().count()).unwrap_or(u32::MAX - 1) + 1;

        (line, col)
    }

    /// Get the byte offset of a line start (1-based line number).
    ///
    /// Returns `None` if the line number is out of range.
    pub fn line_start_offset(&self, line: u32) -> Option<u32> {
        if line == 0 {
            return None;
        }
        self.offsets.get((line - 1) as usize).copied()
    }

    /// Get the byte offset of a line's end (exclusive, before `\n`).
    ///
    /// Takes a 1-based line number. Returns the byte offset just past the
    /// last character of the line (excluding the trailing newline).
    pub fn line_end_offset(&self, source: &str, line: u32) -> Option<u32> {
        // Verify line exists
        self.line_start_offset(line)?;
        // End is either next line's start - 1 (the \n), or source end
        let end = self
            .line_start_offset(line + 1)
            .map_or(source.len(), |o| (o as usize).saturating_sub(1));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "source files are limited to u32::MAX bytes"
        )]
        Some(end as u32)
    }

    /// Extract the text of a 1-based line number (without trailing newline).
    pub fn line_text<'a>(&self, source: &'a str, line: u32) -> Option<&'a str> {
        let start = self.line_start_offset(line)? as usize;
        let end = self.line_end_offset(source, line)? as usize;
        Some(&source[start..end])
    }

    /// Get the number of lines in the source.
    pub fn line_count(&self) -> usize {
        self.offsets.len()
    }
}

/// Compute the 1-based line number from a span and source text.
///
/// Returns the line number where the span starts.
///
/// Note: For repeated lookups, use [`LineOffsetTable`] instead.
pub fn line_number(source: &str, span: Span) -> u32 {
    line_from_offset(source, span.start)
}

/// Compute 1-based line number from a byte offset.
///
/// Counts newlines before the offset to determine the line.
///
/// Note: For repeated lookups, use [`LineOffsetTable`] instead.
pub fn line_from_offset(source: &str, offset: u32) -> u32 {
    let offset = offset as usize;
    let bytes = source.as_bytes();
    let mut line = 1u32;

    for (i, &byte) in bytes.iter().enumerate() {
        if i >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
        }
    }

    line
}

/// Compute 1-based (line, column) from a byte offset.
///
/// The column is computed as the number of characters (not bytes)
/// from the start of the line.
///
/// Note: For repeated lookups, use [`LineOffsetTable`] instead.
pub fn offset_to_line_col(source: &str, offset: u32) -> (u32, u32) {
    let offset = offset as usize;
    let bytes = source.as_bytes();
    let mut line = 1u32;
    let mut line_start = 0usize;

    for (i, &byte) in bytes.iter().enumerate() {
        if i >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }

    // Column is 1-based, counting characters from line start to offset
    let col_bytes = &source[line_start..offset.min(source.len())];
    let col = u32::try_from(col_bytes.chars().count()).unwrap_or(u32::MAX - 1) + 1;

    (line, col)
}

#[cfg(test)]
mod tests;
