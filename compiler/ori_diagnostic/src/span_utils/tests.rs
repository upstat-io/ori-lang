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
#[expect(
    clippy::cast_possible_truncation,
    reason = "test source string is small"
)]
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

// --- line_end_offset / line_text tests ---

#[test]
fn test_line_end_offset_single_line() {
    let source = "hello world";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_end_offset(source, 1), Some(11));
    assert_eq!(table.line_end_offset(source, 2), None);
}

#[test]
fn test_line_end_offset_multiple_lines() {
    let source = "line1\nline2\nline3";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_end_offset(source, 1), Some(5)); // before \n
    assert_eq!(table.line_end_offset(source, 2), Some(11)); // before \n
    assert_eq!(table.line_end_offset(source, 3), Some(17)); // end of source
}

#[test]
fn test_line_end_offset_trailing_newline() {
    let source = "line1\nline2\n";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_end_offset(source, 1), Some(5));
    assert_eq!(table.line_end_offset(source, 2), Some(11));
    assert_eq!(table.line_end_offset(source, 3), Some(12)); // empty line at end
}

#[test]
fn test_line_end_offset_empty_source() {
    let source = "";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_end_offset(source, 1), Some(0));
}

#[test]
fn test_line_end_offset_zero_line() {
    let source = "test";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_end_offset(source, 0), None);
}

#[test]
fn test_line_text_single_line() {
    let source = "hello world";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_text(source, 1), Some("hello world"));
}

#[test]
fn test_line_text_multiple_lines() {
    let source = "first\nsecond\nthird";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_text(source, 1), Some("first"));
    assert_eq!(table.line_text(source, 2), Some("second"));
    assert_eq!(table.line_text(source, 3), Some("third"));
}

#[test]
fn test_line_text_empty_line() {
    let source = "before\n\nafter";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_text(source, 1), Some("before"));
    assert_eq!(table.line_text(source, 2), Some(""));
    assert_eq!(table.line_text(source, 3), Some("after"));
}

#[test]
fn test_line_text_out_of_range() {
    let source = "hello";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_text(source, 0), None);
    assert_eq!(table.line_text(source, 2), None);
}

#[test]
fn test_line_text_unicode() {
    let source = "αβγ\nδε";
    let table = LineOffsetTable::build(source);
    assert_eq!(table.line_text(source, 1), Some("αβγ"));
    assert_eq!(table.line_text(source, 2), Some("δε"));
}
