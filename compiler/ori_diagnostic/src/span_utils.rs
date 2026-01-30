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
mod tests {
    use super::*;

    #[test]
    fn test_line_from_offset_single_line() {
        let source = "hello world";
        assert_eq!(line_from_offset(source, 0), 1);
        assert_eq!(line_from_offset(source, 5), 1);
        assert_eq!(line_from_offset(source, 10), 1);
    }

    #[test]
    fn test_line_from_offset_multiple_lines() {
        let source = "line1\nline2\nline3";
        assert_eq!(line_from_offset(source, 0), 1); // 'l' of line1
        assert_eq!(line_from_offset(source, 5), 1); // '\n' after line1
        assert_eq!(line_from_offset(source, 6), 2); // 'l' of line2
        assert_eq!(line_from_offset(source, 11), 2); // '\n' after line2
        assert_eq!(line_from_offset(source, 12), 3); // 'l' of line3
    }

    #[test]
    fn test_line_number_from_span() {
        let source = "line1\nline2\nline3";
        assert_eq!(line_number(source, Span::new(0, 5)), 1);
        assert_eq!(line_number(source, Span::new(6, 11)), 2);
        assert_eq!(line_number(source, Span::new(12, 17)), 3);
    }

    #[test]
    fn test_offset_to_line_col() {
        let source = "abc\ndefgh\nij";
        // Line 1
        assert_eq!(offset_to_line_col(source, 0), (1, 1)); // 'a'
        assert_eq!(offset_to_line_col(source, 2), (1, 3)); // 'c'
                                                           // Line 2
        assert_eq!(offset_to_line_col(source, 4), (2, 1)); // 'd'
        assert_eq!(offset_to_line_col(source, 7), (2, 4)); // 'g'
                                                           // Line 3
        assert_eq!(offset_to_line_col(source, 10), (3, 1)); // 'i'
    }

    #[test]
    fn test_offset_to_line_col_empty() {
        let source = "";
        assert_eq!(offset_to_line_col(source, 0), (1, 1));
    }

    #[test]
    fn test_offset_to_line_col_unicode() {
        let source = "αβγ\nδε";
        // Greek letters are 2 bytes each
        assert_eq!(offset_to_line_col(source, 0), (1, 1)); // 'α'
        assert_eq!(offset_to_line_col(source, 2), (1, 2)); // 'β'
        assert_eq!(offset_to_line_col(source, 4), (1, 3)); // 'γ'
        assert_eq!(offset_to_line_col(source, 7), (2, 1)); // 'δ' (after \n at byte 6)
    }

    #[test]
    fn test_line_offset_table_build_single_line() {
        let source = "hello world";
        let table = LineOffsetTable::build(source);
        assert_eq!(table.line_count(), 1);
        assert_eq!(table.line_start_offset(1), Some(0));
        assert_eq!(table.line_start_offset(2), None);
    }

    #[test]
    fn test_line_offset_table_build_multiple_lines() {
        let source = "line1\nline2\nline3";
        let table = LineOffsetTable::build(source);
        assert_eq!(table.line_count(), 3);
        assert_eq!(table.line_start_offset(1), Some(0));
        assert_eq!(table.line_start_offset(2), Some(6));
        assert_eq!(table.line_start_offset(3), Some(12));
        assert_eq!(table.line_start_offset(4), None);
    }

    #[test]
    fn test_line_offset_table_line_from_offset_single_line() {
        let source = "hello world";
        let table = LineOffsetTable::build(source);
        assert_eq!(table.line_from_offset(0), 1);
        assert_eq!(table.line_from_offset(5), 1);
        assert_eq!(table.line_from_offset(10), 1);
    }

    #[test]
    fn test_line_offset_table_line_from_offset_multiple_lines() {
        let source = "line1\nline2\nline3";
        let table = LineOffsetTable::build(source);
        assert_eq!(table.line_from_offset(0), 1); // 'l' of line1
        assert_eq!(table.line_from_offset(5), 1); // '\n' after line1
        assert_eq!(table.line_from_offset(6), 2); // 'l' of line2
        assert_eq!(table.line_from_offset(11), 2); // '\n' after line2
        assert_eq!(table.line_from_offset(12), 3); // 'l' of line3
    }

    #[test]
    fn test_line_offset_table_offset_to_line_col() {
        let source = "abc\ndefgh\nij";
        let table = LineOffsetTable::build(source);
        // Line 1
        assert_eq!(table.offset_to_line_col(source, 0), (1, 1)); // 'a'
        assert_eq!(table.offset_to_line_col(source, 2), (1, 3)); // 'c'
                                                                 // Line 2
        assert_eq!(table.offset_to_line_col(source, 4), (2, 1)); // 'd'
        assert_eq!(table.offset_to_line_col(source, 7), (2, 4)); // 'g'
                                                                 // Line 3
        assert_eq!(table.offset_to_line_col(source, 10), (3, 1)); // 'i'
    }

    #[test]
    fn test_line_offset_table_empty_source() {
        let source = "";
        let table = LineOffsetTable::build(source);
        assert_eq!(table.line_count(), 1);
        assert_eq!(table.offset_to_line_col(source, 0), (1, 1));
    }

    #[test]
    fn test_line_offset_table_unicode() {
        let source = "αβγ\nδε";
        let table = LineOffsetTable::build(source);
        // Greek letters are 2 bytes each
        assert_eq!(table.offset_to_line_col(source, 0), (1, 1)); // 'α'
        assert_eq!(table.offset_to_line_col(source, 2), (1, 2)); // 'β'
        assert_eq!(table.offset_to_line_col(source, 4), (1, 3)); // 'γ'
        assert_eq!(table.offset_to_line_col(source, 7), (2, 1)); // 'δ' (after \n at byte 6)
    }

    #[test]
    fn test_line_offset_table_matches_linear_scan() {
        // Verify that LineOffsetTable produces identical results to linear scan
        let source = "first line\nsecond longer line\n\nfourth after empty\nlast";
        let table = LineOffsetTable::build(source);

        for offset in 0..source.len() as u32 {
            let table_result = table.offset_to_line_col(source, offset);
            let linear_result = offset_to_line_col(source, offset);
            assert_eq!(
                table_result, linear_result,
                "Mismatch at offset {offset}: table={table_result:?}, linear={linear_result:?}"
            );
        }
    }

    #[test]
    fn test_line_offset_table_trailing_newline() {
        let source = "line1\nline2\n";
        let table = LineOffsetTable::build(source);
        assert_eq!(table.line_count(), 3); // Empty line after trailing \n
        assert_eq!(table.line_from_offset(12), 3); // After second \n
    }

    #[test]
    fn test_line_offset_table_line_start_offset_zero() {
        let table = LineOffsetTable::build("test");
        assert_eq!(table.line_start_offset(0), None); // Line 0 doesn't exist
    }
}
