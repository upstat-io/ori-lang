use crate::SourceBuffer;

// === Basic Navigation ===

#[test]
fn current_returns_first_byte() {
    let buf = SourceBuffer::new("abc");
    let cursor = buf.cursor();
    assert_eq!(cursor.current(), b'a');
}

#[test]
fn advance_moves_forward() {
    let buf = SourceBuffer::new("abc");
    let mut cursor = buf.cursor();
    cursor.advance();
    assert_eq!(cursor.current(), b'b');
    assert_eq!(cursor.pos(), 1);
}

#[test]
fn advance_n_moves_multiple() {
    let buf = SourceBuffer::new("abcdef");
    let mut cursor = buf.cursor();
    cursor.advance_n(3);
    assert_eq!(cursor.current(), b'd');
    assert_eq!(cursor.pos(), 3);
}

#[test]
fn advance_through_entire_source() {
    let buf = SourceBuffer::new("hi");
    let mut cursor = buf.cursor();
    assert_eq!(cursor.current(), b'h');
    cursor.advance();
    assert_eq!(cursor.current(), b'i');
    cursor.advance();
    assert!(cursor.is_eof());
}

// === Peek ===

#[test]
fn peek_returns_next_byte() {
    let buf = SourceBuffer::new("abc");
    let cursor = buf.cursor();
    assert_eq!(cursor.peek(), b'b');
}

#[test]
fn peek2_returns_two_ahead() {
    let buf = SourceBuffer::new("abc");
    let cursor = buf.cursor();
    assert_eq!(cursor.peek2(), b'c');
}

#[test]
fn peek_near_end_returns_sentinel() {
    let buf = SourceBuffer::new("ab");
    let mut cursor = buf.cursor();
    cursor.advance(); // at 'b'
    assert_eq!(cursor.peek(), 0); // sentinel
}

#[test]
fn peek2_near_end_returns_zero() {
    let buf = SourceBuffer::new("a");
    let cursor = buf.cursor();
    // current='a', peek=sentinel(0), peek2=padding(0)
    assert_eq!(cursor.peek2(), 0);
}

// === EOF Detection ===

#[test]
fn is_eof_at_sentinel() {
    let buf = SourceBuffer::new("x");
    let mut cursor = buf.cursor();
    assert!(!cursor.is_eof());
    cursor.advance(); // past 'x', at sentinel
    assert!(cursor.is_eof());
}

#[test]
fn is_eof_on_empty_source() {
    let buf = SourceBuffer::new("");
    let cursor = buf.cursor();
    assert!(cursor.is_eof());
}

#[test]
fn interior_null_is_not_eof() {
    let buf = SourceBuffer::new("a\0b");
    let mut cursor = buf.cursor();
    cursor.advance(); // at '\0' (interior null)
    assert_eq!(cursor.current(), 0);
    assert!(!cursor.is_eof()); // pos=1 < source_len=3
    cursor.advance(); // at 'b'
    assert_eq!(cursor.current(), b'b');
}

// === Slice ===

#[test]
fn slice_extracts_substring() {
    let buf = SourceBuffer::new("hello world");
    let cursor = buf.cursor();
    assert_eq!(cursor.slice(0, 5), "hello");
    assert_eq!(cursor.slice(6, 11), "world");
}

#[test]
fn slice_from_extracts_to_current() {
    let buf = SourceBuffer::new("abcdef");
    let mut cursor = buf.cursor();
    cursor.advance_n(3); // pos = 3
    assert_eq!(cursor.slice_from(0), "abc");
    assert_eq!(cursor.slice_from(1), "bc");
}

#[test]
fn slice_empty_range() {
    let buf = SourceBuffer::new("hello");
    let cursor = buf.cursor();
    assert_eq!(cursor.slice(2, 2), "");
}

#[test]
fn slice_utf8_multibyte() {
    let source = "hi \u{1F600} bye"; // emoji is 4 bytes
    let buf = SourceBuffer::new(source);
    let cursor = buf.cursor();
    // "hi " = 3 bytes, emoji = 4 bytes, " bye" = 4 bytes
    assert_eq!(cursor.slice(0, 3), "hi ");
    assert_eq!(cursor.slice(7, 11), " bye");
}

// === eat_while ===

#[test]
fn eat_while_consumes_matching_bytes() {
    let buf = SourceBuffer::new("aaabbb");
    let mut cursor = buf.cursor();
    cursor.eat_while(|b| b == b'a');
    assert_eq!(cursor.pos(), 3);
    assert_eq!(cursor.current(), b'b');
}

#[test]
fn eat_while_stops_at_sentinel() {
    let buf = SourceBuffer::new("aaa");
    let mut cursor = buf.cursor();
    cursor.eat_while(|b| b == b'a');
    assert_eq!(cursor.pos(), 3);
    assert!(cursor.is_eof());
}

#[test]
fn eat_while_whitespace() {
    let buf = SourceBuffer::new("   hello");
    let mut cursor = buf.cursor();
    cursor.eat_while(|b| b == b' ' || b == b'\t');
    assert_eq!(cursor.pos(), 3);
    assert_eq!(cursor.current(), b'h');
}

#[test]
fn eat_while_no_match() {
    let buf = SourceBuffer::new("hello");
    let mut cursor = buf.cursor();
    cursor.eat_while(|b| b == b'z');
    assert_eq!(cursor.pos(), 0); // didn't move
}

// === eat_until ===

#[test]
fn eat_until_finds_target() {
    let buf = SourceBuffer::new("hello world");
    let mut cursor = buf.cursor();
    let consumed = cursor.eat_until(b' ');
    assert_eq!(consumed, 5);
    assert_eq!(cursor.current(), b' ');
}

#[test]
fn eat_until_stops_at_eof() {
    let buf = SourceBuffer::new("hello");
    let mut cursor = buf.cursor();
    let consumed = cursor.eat_until(b'z'); // not found
    assert_eq!(consumed, 5);
    assert!(cursor.is_eof());
}

#[test]
fn eat_until_at_target_consumes_zero() {
    let buf = SourceBuffer::new("xhello");
    let mut cursor = buf.cursor();
    let consumed = cursor.eat_until(b'x');
    assert_eq!(consumed, 0);
    assert_eq!(cursor.current(), b'x');
}

#[test]
fn eat_until_skips_interior_null() {
    let buf = SourceBuffer::new("a\0b\0c!");
    let mut cursor = buf.cursor();
    let consumed = cursor.eat_until(b'!');
    assert_eq!(consumed, 5);
    assert_eq!(cursor.current(), b'!');
}

// === Copy Semantics ===

#[test]
fn cursor_is_copy_for_checkpointing() {
    let buf = SourceBuffer::new("abcdef");
    let mut cursor = buf.cursor();
    cursor.advance_n(2);

    // Snapshot via Copy
    let saved = cursor;

    // Advance original
    cursor.advance_n(3);
    assert_eq!(cursor.pos(), 5);

    // Saved is still at old position
    assert_eq!(saved.pos(), 2);
    assert_eq!(saved.current(), b'c');
}

// === eat_until_newline_or_eof ===

#[test]
fn eat_until_newline_finds_lf() {
    let buf = SourceBuffer::new("hello\nworld");
    let mut cursor = buf.cursor();
    cursor.eat_until_newline_or_eof();
    assert_eq!(cursor.pos(), 5);
    assert_eq!(cursor.current(), b'\n');
}

#[test]
fn eat_until_newline_stops_at_eof() {
    let buf = SourceBuffer::new("no newline here");
    let mut cursor = buf.cursor();
    cursor.eat_until_newline_or_eof();
    assert_eq!(cursor.pos(), 15);
    assert!(cursor.is_eof());
}

#[test]
fn eat_until_newline_empty_source() {
    let buf = SourceBuffer::new("");
    let mut cursor = buf.cursor();
    cursor.eat_until_newline_or_eof();
    assert!(cursor.is_eof());
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn eat_until_newline_at_first_position() {
    let buf = SourceBuffer::new("\nhello");
    let mut cursor = buf.cursor();
    cursor.eat_until_newline_or_eof();
    assert_eq!(cursor.pos(), 0);
    assert_eq!(cursor.current(), b'\n');
}

#[test]
fn eat_until_newline_single_byte() {
    let buf = SourceBuffer::new("x");
    let mut cursor = buf.cursor();
    cursor.eat_until_newline_or_eof();
    assert_eq!(cursor.pos(), 1);
    assert!(cursor.is_eof());
}

#[test]
fn eat_until_newline_from_middle() {
    let buf = SourceBuffer::new("// comment\nnext");
    let mut cursor = buf.cursor();
    cursor.advance_n(3); // skip "// "
    cursor.eat_until_newline_or_eof();
    assert_eq!(cursor.pos(), 10);
    assert_eq!(cursor.current(), b'\n');
}

// === skip_to_string_delim ===

#[test]
fn skip_to_string_delim_finds_closing_quote() {
    let buf = SourceBuffer::new("hello\"rest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, b'"');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_string_delim_finds_backslash() {
    let buf = SourceBuffer::new("hello\\nrest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, b'\\');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_string_delim_finds_newline() {
    let buf = SourceBuffer::new("hello\nrest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, b'\n');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_string_delim_finds_cr() {
    let buf = SourceBuffer::new("hello\rrest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, b'\r');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_string_delim_returns_earliest() {
    // backslash before quote
    let buf = SourceBuffer::new("abc\\\"rest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, b'\\');
    assert_eq!(cursor.pos(), 3);
}

#[test]
fn skip_to_string_delim_eof() {
    let buf = SourceBuffer::new("hello");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, 0);
    assert!(cursor.is_eof());
}

#[test]
fn skip_to_string_delim_empty() {
    let buf = SourceBuffer::new("");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, 0);
    assert!(cursor.is_eof());
}

#[test]
fn skip_to_string_delim_at_first_position() {
    let buf = SourceBuffer::new("\"hello");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, b'"');
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn skip_to_string_delim_cr_before_newline() {
    // \r appears before \n — should find \r first
    let buf = SourceBuffer::new("abc\r\nrest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_string_delim();
    assert_eq!(b, b'\r');
    assert_eq!(cursor.pos(), 3);
}

// === skip_to_template_delim ===

#[test]
fn skip_to_template_delim_finds_backtick() {
    let buf = SourceBuffer::new("hello`rest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'`');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_template_delim_finds_open_brace() {
    let buf = SourceBuffer::new("hello{rest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'{');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_template_delim_finds_close_brace() {
    let buf = SourceBuffer::new("hello}rest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'}');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_template_delim_finds_backslash() {
    let buf = SourceBuffer::new("hello\\rest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'\\');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_template_delim_finds_newline() {
    let buf = SourceBuffer::new("hello\nrest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'\n');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_template_delim_finds_cr() {
    let buf = SourceBuffer::new("hello\rrest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'\r');
    assert_eq!(cursor.pos(), 5);
}

#[test]
fn skip_to_template_delim_returns_earliest() {
    // backslash before backtick
    let buf = SourceBuffer::new("abc\\`rest");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'\\');
    assert_eq!(cursor.pos(), 3);
}

#[test]
fn skip_to_template_delim_eof() {
    let buf = SourceBuffer::new("hello");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, 0);
    assert!(cursor.is_eof());
}

#[test]
fn skip_to_template_delim_empty() {
    let buf = SourceBuffer::new("");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, 0);
    assert!(cursor.is_eof());
}

#[test]
fn skip_to_template_delim_at_first_position() {
    let buf = SourceBuffer::new("`hello");
    let mut cursor = buf.cursor();
    let b = cursor.skip_to_template_delim();
    assert_eq!(b, b'`');
    assert_eq!(cursor.pos(), 0);
}

// === eat_whitespace (SWAR) ===

#[test]
fn eat_whitespace_spaces_only() {
    let buf = SourceBuffer::new("    hello");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 4);
    assert_eq!(cursor.current(), b'h');
}

#[test]
fn eat_whitespace_tabs_only() {
    let buf = SourceBuffer::new("\t\t\thello");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 3);
    assert_eq!(cursor.current(), b'h');
}

#[test]
fn eat_whitespace_mixed_spaces_and_tabs() {
    let buf = SourceBuffer::new("  \t \t  x");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    // "  \t \t  " = 7 bytes of whitespace before 'x'
    assert_eq!(cursor.pos(), 7);
    assert_eq!(cursor.current(), b'x');
}

#[test]
fn eat_whitespace_no_whitespace() {
    let buf = SourceBuffer::new("hello");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 0); // didn't move
}

#[test]
fn eat_whitespace_empty_source() {
    let buf = SourceBuffer::new("");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 0);
    assert!(cursor.is_eof());
}

#[test]
fn eat_whitespace_all_whitespace() {
    let buf = SourceBuffer::new("   \t\t   ");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 8);
    assert!(cursor.is_eof());
}

#[test]
fn eat_whitespace_from_middle() {
    let buf = SourceBuffer::new("abc   def");
    let mut cursor = buf.cursor();
    cursor.advance_n(3); // skip "abc"
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 6);
    assert_eq!(cursor.current(), b'd');
}

#[test]
fn eat_whitespace_newline_stops() {
    // Newlines are NOT horizontal whitespace — should stop at \n
    let buf = SourceBuffer::new("   \nhello");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 3);
    assert_eq!(cursor.current(), b'\n');
}

#[test]
fn eat_whitespace_cr_stops() {
    // Carriage return is NOT consumed by eat_whitespace
    let buf = SourceBuffer::new("  \rhello");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 2);
    assert_eq!(cursor.current(), b'\r');
}

#[test]
fn eat_whitespace_long_run_16_bytes() {
    // 16 spaces = exercises SWAR for 2 full chunks
    let buf = SourceBuffer::new("                x");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 16);
    assert_eq!(cursor.current(), b'x');
}

#[test]
fn eat_whitespace_long_run_exactly_8() {
    // Exactly 8 spaces = one full SWAR chunk
    let buf = SourceBuffer::new("        x");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 8);
    assert_eq!(cursor.current(), b'x');
}

#[test]
fn eat_whitespace_7_bytes() {
    // 7 spaces = pure scalar tail (no SWAR chunks)
    let buf = SourceBuffer::new("       x");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 7);
    assert_eq!(cursor.current(), b'x');
}

#[test]
fn eat_whitespace_9_bytes() {
    // 9 spaces = one SWAR chunk + 1 scalar byte
    let buf = SourceBuffer::new("         x");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 9);
    assert_eq!(cursor.current(), b'x');
}

#[test]
fn eat_whitespace_sentinel_stops() {
    // Only whitespace then EOF — sentinel (0x00) stops scanning
    let buf = SourceBuffer::new("     ");
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    assert_eq!(cursor.pos(), 5);
    assert!(cursor.is_eof());
}

#[test]
fn eat_whitespace_long_mixed_run() {
    // 20 bytes of mixed spaces and tabs
    let source = " \t \t \t \t \t \t \t \t \t \tx";
    let buf = SourceBuffer::new(source);
    let mut cursor = buf.cursor();
    cursor.eat_whitespace();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "test string is under 30 bytes"
    )]
    let expected = source.len() as u32 - 1; // everything except 'x'
    assert_eq!(cursor.pos(), expected);
    assert_eq!(cursor.current(), b'x');
}

// === SWAR vs scalar agreement ===

#[test]
fn swar_matches_scalar_basic_cases() {
    use super::{scalar_count_whitespace, swar_count_whitespace};

    let cases: &[&[u8]] = &[
        b"",
        b" ",
        b"\t",
        b"  ",
        b"\t\t",
        b" \t \t",
        b"hello",
        b"   hello",
        b"\t\thello",
        b"        ",         // 8 spaces
        b"         ",        // 9 spaces
        b"       ",          // 7 spaces
        b"                ", // 16 spaces
        b"   \nhello",
        b"   \rhello",
        b"\x00",
        b" \x00 ",
    ];

    for case in cases {
        let scalar = scalar_count_whitespace(case);
        let swar = swar_count_whitespace(case);
        assert_eq!(scalar, swar, "scalar={scalar} != swar={swar} for {case:?}",);
    }
}

// === Property tests ===

#[allow(
    clippy::disallowed_types,
    reason = "proptest macros internally use Arc"
)]
mod proptest_swar {
    use super::super::{scalar_count_whitespace, swar_count_whitespace};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn swar_matches_scalar_random(bytes in proptest::collection::vec(any::<u8>(), 0..256)) {
            let scalar = scalar_count_whitespace(&bytes);
            let swar = swar_count_whitespace(&bytes);
            prop_assert_eq!(scalar, swar, "mismatch for {} bytes", bytes.len());
        }

        #[test]
        fn swar_matches_scalar_whitespace_heavy(
            bytes in proptest::collection::vec(
                prop_oneof![
                    Just(b' '),
                    Just(b'\t'),
                    Just(b'a'),
                    Just(b'\n'),
                    Just(b'\0'),
                ],
                0..256,
            )
        ) {
            let scalar = scalar_count_whitespace(&bytes);
            let swar = swar_count_whitespace(&bytes);
            prop_assert_eq!(scalar, swar, "mismatch for {} bytes", bytes.len());
        }

        #[test]
        fn swar_matches_scalar_mostly_spaces(
            prefix_len in 0usize..128,
            suffix in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            let mut bytes = vec![b' '; prefix_len];
            bytes.extend_from_slice(&suffix);
            let scalar = scalar_count_whitespace(&bytes);
            let swar = swar_count_whitespace(&bytes);
            prop_assert_eq!(scalar, swar, "mismatch for {} bytes", bytes.len());
        }
    }
}
