//! Zero-cost cursor over a sentinel-terminated buffer.
//!
//! The cursor advances through the buffer byte-by-byte. EOF is detected
//! when the current byte equals the sentinel (`0x00`) and the position
//! has reached or exceeded the source length. No explicit bounds checking
//! is performed in the common case -- the sentinel guarantees safe termination.
//!
//! # Interior Null Bytes
//!
//! If the source contains interior null bytes (U+0000), the cursor
//! distinguishes them from EOF by comparing `pos` against `source_len`.
//! A null at `pos < source_len` is an interior null (error token);
//! a null at `pos >= source_len` is the sentinel (EOF).

/// Returns the earliest (minimum) of two optional positions.
///
/// Used by the memchr-based scanning methods to combine results from
/// separate memchr calls when we need to search for more bytes than
/// `memchr3` supports (which handles at most 3 needles).
fn earliest_of(a: Option<usize>, b: Option<usize>) -> Option<usize> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

/// Count leading whitespace bytes (space `0x20` or tab `0x09`) using scalar loop.
///
/// Reference implementation for property testing. Returns the number of
/// consecutive whitespace bytes from the start of `buf`.
#[cfg(test)]
fn scalar_count_whitespace(buf: &[u8]) -> usize {
    buf.iter().take_while(|&&b| b == b' ' || b == b'\t').count()
}

/// Count leading whitespace bytes (space `0x20` or tab `0x09`) using SWAR.
///
/// Processes 8 bytes at a time by loading them as a little-endian `u64` and
/// using carry-free zero-byte detection to find the first non-whitespace byte.
/// Falls back to scalar for the remaining 0–7 byte tail.
///
/// The sentinel byte (`0x00`) is neither space nor tab, so scanning terminates
/// naturally at EOF without explicit bounds checking beyond the `i + 8 <= len`
/// guard for the SWAR loop.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "reference impl kept for property testing against scalar version"
    )
)]
#[allow(
    unsafe_code,
    reason = "unaligned u64 reads required for SWAR byte-parallel processing"
)]
fn swar_count_whitespace(buf: &[u8]) -> usize {
    /// Detects which bytes in a `u64` are zero, returning a mask with the high
    /// bit (`0x80`) set in each zero byte lane.
    ///
    /// Uses carry-free detection: masks each byte to 7 bits, adds `0x7F` per
    /// lane (max result `0xFE`, no carry across bytes), then ORs with the
    /// original to catch `0x80`. Inverts to get zero-byte positions.
    ///
    /// This avoids the borrow-propagation bug in Mycroft's formula, where
    /// `(v - 0x0101..01)` can carry between adjacent byte lanes.
    #[inline]
    const fn byte_zero_mask(v: u64) -> u64 {
        const LO7: u64 = 0x7F7F_7F7F_7F7F_7F7F;
        const HI: u64 = 0x8080_8080_8080_8080;
        // (v & 0x7F..7F) + 0x7F..7F: high bit set in each lane where masked byte >= 1.
        // Max per-byte value: 0x7F + 0x7F = 0xFE — no carry into adjacent byte.
        // OR with v: also catches bytes where only bit 7 was set (0x80).
        // Invert: high bit set where byte WAS zero.
        !((v & LO7).wrapping_add(LO7) | v) & HI
    }

    const SPACES: u64 = 0x2020_2020_2020_2020;
    const TABS: u64 = 0x0909_0909_0909_0909;
    const HI: u64 = 0x8080_8080_8080_8080;

    let len = buf.len();
    let mut i = 0;

    // SWAR loop: process 8 bytes at a time.
    while i + 8 <= len {
        // SAFETY: We verified `i + 8 <= len`, so `buf[i..i+8]` is in bounds.
        // We use `read_unaligned` because the cursor position is not guaranteed
        // to be 8-byte aligned.
        let chunk = unsafe { buf.as_ptr().add(i).cast::<u64>().read_unaligned() };

        // XOR with space pattern: zero bytes where byte IS a space.
        let xor_space = chunk ^ SPACES;
        // XOR with tab pattern: zero bytes where byte IS a tab.
        let xor_tab = chunk ^ TABS;

        // Detect zero bytes in each XOR result (= positions that matched).
        let space_mask = byte_zero_mask(xor_space);
        let tab_mask = byte_zero_mask(xor_tab);

        // Combine: high bit set where byte IS whitespace (space OR tab).
        let ws_mask = space_mask | tab_mask;

        // Invert: high bit set where byte is NOT whitespace.
        let non_ws = !ws_mask & HI;

        if non_ws != 0 {
            // Found a non-whitespace byte. Its position (in bytes) is
            // trailing_zeros / 8 (each byte lane is 8 bits wide).
            return i + (non_ws.trailing_zeros() as usize / 8);
        }

        // All 8 bytes were whitespace — continue.
        i += 8;
    }

    // Scalar tail: process remaining 0–7 bytes.
    while i < len {
        let b = buf[i];
        if b != b' ' && b != b'\t' {
            return i;
        }
        i += 1;
    }

    i
}

/// Zero-cost cursor over a sentinel-terminated byte buffer.
///
/// Created via [`SourceBuffer::cursor()`](crate::SourceBuffer::cursor).
/// The cursor is [`Copy`], enabling cheap state snapshots for backtracking.
///
/// # Invariant
///
/// `buf` must be sentinel-terminated: `buf[source_len] == 0x00`, and all
/// bytes after `source_len` are `0x00` (cache-line padding). This is
/// guaranteed by [`SourceBuffer`](crate::SourceBuffer) construction.
#[derive(Clone, Copy, Debug)]
pub struct Cursor<'a> {
    /// Sentinel-terminated buffer (source + sentinel + padding).
    buf: &'a [u8],
    /// Current read position (byte index into `buf`).
    pos: u32,
    /// Length of actual source content (excludes sentinel and padding).
    source_len: u32,
}

/// Size assertion: Cursor should be <= 24 bytes on 64-bit platforms.
/// &[u8] = 16 (fat pointer), u32 = 4, u32 = 4 => 24 bytes.
const _: () = assert!(std::mem::size_of::<Cursor<'static>>() <= 24);

impl<'a> Cursor<'a> {
    /// Create a new cursor at position 0 over a sentinel-terminated buffer.
    ///
    /// # Contract
    ///
    /// `buf[source_len]` must be `0x00` (sentinel). All bytes after the
    /// sentinel must also be `0x00` (padding). This is guaranteed by
    /// `SourceBuffer::new()`.
    pub(crate) fn new(buf: &'a [u8], source_len: u32) -> Self {
        debug_assert!(
            (source_len as usize) < buf.len(),
            "sentinel must be within buffer bounds"
        );
        debug_assert!(buf[source_len as usize] == 0, "sentinel byte must be 0x00");
        Self {
            buf,
            pos: 0,
            source_len,
        }
    }

    /// Returns the byte at the current position.
    ///
    /// Returns `0x00` when at EOF (the sentinel byte). Interior null bytes
    /// also return `0x00`; use [`is_eof()`](Self::is_eof) to distinguish.
    #[inline]
    pub fn current(&self) -> u8 {
        self.buf[self.pos as usize]
    }

    /// Returns the byte one position ahead of current.
    ///
    /// Safe to call at any position: the sentinel and cache-line padding
    /// guarantee valid reads beyond the source content.
    #[inline]
    pub fn peek(&self) -> u8 {
        self.buf[self.pos as usize + 1]
    }

    /// Returns the byte two positions ahead of current.
    ///
    /// Safe to call at any position: cache-line alignment provides at least
    /// one full cache line of zero padding after the sentinel.
    #[inline]
    pub fn peek2(&self) -> u8 {
        self.buf[self.pos as usize + 2]
    }

    /// Advance the cursor by one byte.
    #[inline]
    pub fn advance(&mut self) {
        self.pos += 1;
    }

    /// Advance the cursor by `n` bytes.
    #[inline]
    pub fn advance_n(&mut self, n: u32) {
        self.pos += n;
    }

    /// Returns `true` if the cursor has reached EOF.
    ///
    /// EOF is when the current byte is the sentinel (`0x00`) and the
    /// position is at or past the source length. This distinguishes
    /// EOF from interior null bytes.
    #[inline]
    pub fn is_eof(&self) -> bool {
        self.current() == 0 && self.pos >= self.source_len
    }

    /// Current byte offset in the source.
    #[inline]
    pub fn pos(&self) -> u32 {
        self.pos
    }

    /// Length of the source content (excludes sentinel and padding).
    #[inline]
    pub fn source_len(&self) -> u32 {
        self.source_len
    }

    /// Extract a source substring as `&str`.
    ///
    /// # Contract
    ///
    /// `start..end` must fall within the source content (`end <= source_len`)
    /// and on valid UTF-8 character boundaries. This is guaranteed when
    /// `start` and `end` come from the scanner's token boundary tracking,
    /// since the source was originally valid UTF-8 (`&str`).
    #[allow(
        unsafe_code,
        reason = "from_utf8_unchecked on source originally validated as &str"
    )]
    pub fn slice(&self, start: u32, end: u32) -> &'a str {
        debug_assert!(
            end <= self.source_len,
            "slice end {end} exceeds source length {}",
            self.source_len
        );
        debug_assert!(start <= end, "slice start {start} exceeds end {end}");
        // SAFETY: The source buffer was constructed from `&str` (valid UTF-8).
        // The scanner ensures start..end falls on character boundaries within
        // the source content.
        unsafe { std::str::from_utf8_unchecked(&self.buf[start as usize..end as usize]) }
    }

    /// Extract a source substring from `start` to the current position.
    ///
    /// Equivalent to `self.slice(start, self.pos())`.
    pub fn slice_from(&self, start: u32) -> &'a str {
        self.slice(start, self.pos)
    }

    /// Advance while `pred` returns `true` for the current byte.
    ///
    /// The sentinel byte (`0x00`) naturally terminates the loop for all
    /// reasonable predicates, as `pred(0)` should return `false`.
    ///
    /// # Contract
    ///
    /// `pred(0)` must return `false`. This is true for all standard byte
    /// classification predicates (`is_ascii_alphanumeric`, `is_ascii_whitespace`,
    /// etc.). If `pred(0)` returns `true`, the cursor advances into the
    /// zero-filled padding region but will eventually stop (all padding is `0x00`,
    /// and Rust's bounds checking prevents out-of-bounds access).
    #[inline]
    pub fn eat_while(&mut self, pred: impl Fn(u8) -> bool) {
        while pred(self.buf[self.pos as usize]) {
            self.pos += 1;
        }
    }

    /// Returns the number of bytes in the UTF-8 character starting with `byte`.
    ///
    /// Uses the leading byte to determine character width:
    /// - `0xC0..=0xDF`: 2 bytes
    /// - `0xE0..=0xEF`: 3 bytes
    /// - `0xF0..=0xF7`: 4 bytes
    /// - Everything else (ASCII, continuation, invalid): 1 byte
    #[inline]
    pub fn utf8_char_width(byte: u8) -> u32 {
        match byte {
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => 1,
        }
    }

    /// Advance the cursor past one full UTF-8 character.
    ///
    /// Uses the current byte as the leading byte to determine how many
    /// bytes to skip. Handles ASCII (1 byte) through 4-byte sequences.
    #[inline]
    pub fn advance_char(&mut self) {
        let width = Self::utf8_char_width(self.current());
        self.advance_n(width);
    }

    /// Advance to the next `\n` byte or EOF using SIMD-accelerated search.
    ///
    /// Used by the comment scanner to skip comment bodies.
    /// Scans only within source content (not into sentinel/padding).
    /// If no newline found, positions cursor at EOF sentinel.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "remaining.len() <= source_len which fits in u32"
    )]
    pub fn eat_until_newline_or_eof(&mut self) {
        let remaining = &self.buf[self.pos as usize..self.source_len as usize];
        if let Some(offset) = memchr::memchr(b'\n', remaining) {
            self.pos += offset as u32;
        } else {
            self.pos = self.source_len;
        }
    }

    /// Advance past ordinary string content to the next interesting byte.
    /// Returns the byte found, or 0 for EOF.
    ///
    /// "Interesting" bytes for strings: `"`, `\`, `\n`, `\r`.
    /// Uses memchr3 for SIMD-accelerated search of the 3 most common
    /// delimiters (`"`, `\`, `\n`), with a secondary check for `\r`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "remaining.len() <= source_len which fits in u32"
    )]
    pub fn skip_to_string_delim(&mut self) -> u8 {
        let remaining = &self.buf[self.pos as usize..self.source_len as usize];
        // Find nearest of ", \, or \n (the 3 most common string terminators)
        let primary = memchr::memchr3(b'"', b'\\', b'\n', remaining);
        // Also check for \r (rare but must be caught; spec: lone CR = newline)
        let cr = memchr::memchr(b'\r', remaining);

        // Take the earliest match
        let offset = earliest_of(primary, cr);

        if let Some(off) = offset {
            self.pos += off as u32;
            self.buf[self.pos as usize]
        } else {
            self.pos = self.source_len;
            0 // EOF sentinel
        }
    }

    /// Advance past ordinary template content to the next interesting byte.
    /// Returns the byte found, or 0 for EOF.
    ///
    /// Template delimiters: `` ` ``, `{`, `}`, `\`, `\n`, `\r`.
    /// Uses memchr3 for the 3 most common (`` ` ``, `{`, `\`),
    /// with secondary search for `}`, `\n`, `\r`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "remaining.len() <= source_len which fits in u32"
    )]
    pub fn skip_to_template_delim(&mut self) -> u8 {
        let remaining = &self.buf[self.pos as usize..self.source_len as usize];
        // Primary: find backtick, open brace, or backslash
        let primary = memchr::memchr3(b'`', b'{', b'\\', remaining);
        // Secondary: find close brace, newline, or carriage return
        let secondary = memchr::memchr3(b'}', b'\n', b'\r', remaining);

        // Take the earliest match
        let offset = earliest_of(primary, secondary);

        if let Some(off) = offset {
            self.pos += off as u32;
            self.buf[self.pos as usize]
        } else {
            self.pos = self.source_len;
            0
        }
    }

    /// Advance past horizontal whitespace (spaces and tabs).
    ///
    /// Uses a simple byte loop which is faster than SWAR for the common case
    /// of short whitespace runs (1-4 bytes typical in source code). The sentinel
    /// byte (`0x00`) naturally terminates scanning since it is neither space
    /// nor tab.
    ///
    /// For long whitespace runs (8+ bytes), [`swar_count_whitespace`] is
    /// available but is not used here because typical source code has short
    /// runs between tokens (1-2 spaces) or indentation (4 spaces).
    #[inline]
    pub fn eat_whitespace(&mut self) {
        loop {
            let b = self.buf[self.pos as usize];
            if b == b' ' || b == b'\t' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Advance until `byte` is found or EOF is reached.
    ///
    /// Returns the number of bytes consumed. The cursor is positioned at the
    /// found byte, or at EOF if the byte was not found.
    ///
    /// Interior null bytes are skipped (they are not EOF).
    pub fn eat_until(&mut self, byte: u8) -> u32 {
        let start = self.pos;
        loop {
            let b = self.buf[self.pos as usize];
            if b == byte {
                break;
            }
            // Distinguish interior null (pos < source_len) from sentinel (pos >= source_len).
            if b == 0 && self.pos >= self.source_len {
                break;
            }
            self.pos += 1;
        }
        self.pos - start
    }
}

#[cfg(test)]
mod tests {
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
}
