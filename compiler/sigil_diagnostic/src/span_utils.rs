//! Span utility functions for diagnostic processing.
//!
//! Provides helpers for computing line and column numbers from spans,
//! used by DiagnosticQueue for sorting and deduplication.

use sigil_ir::Span;

/// Compute the 1-based line number from a span and source text.
///
/// Returns the line number where the span starts.
pub fn line_number(source: &str, span: Span) -> u32 {
    line_from_offset(source, span.start)
}

/// Compute 1-based line number from a byte offset.
///
/// Counts newlines before the offset to determine the line.
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
    let col = col_bytes.chars().count() as u32 + 1;

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
        assert_eq!(line_from_offset(source, 0), 1);  // 'l' of line1
        assert_eq!(line_from_offset(source, 5), 1);  // '\n' after line1
        assert_eq!(line_from_offset(source, 6), 2);  // 'l' of line2
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
}
