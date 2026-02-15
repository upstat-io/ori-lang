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
mod tests;
