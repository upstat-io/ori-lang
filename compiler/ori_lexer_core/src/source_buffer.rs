//! Sentinel-terminated source buffer for zero-bounds-check scanning.
//!
//! The buffer guarantees a `0x00` sentinel byte after the source content,
//! allowing the scanner to detect EOF without explicit bounds checking.
//! The total buffer size is rounded up to the next 64-byte boundary for
//! cache-line alignment, which also provides safe padding for `peek()`
//! and `peek2()` operations near the end of the buffer.
//!
//! # Encoding Detection
//!
//! During construction, the buffer scans for encoding issues:
//! - UTF-8 BOM (forbidden per Ori spec: `02-source-code.md` SS Encoding)
//! - UTF-16 BOMs (wrong encoding)
//! - Interior null bytes (forbidden per grammar: `unicode_char` excludes NUL)
//!
//! Issues are recorded as [`EncodingIssue`] values. The integration layer
//! (`ori_lexer`) converts these to diagnostic errors with spans and messages.

use crate::Cursor;

/// Cache line size in bytes, used for buffer alignment padding.
const CACHE_LINE: usize = 64;

/// Sentinel-terminated source buffer for zero-bounds-check scanning.
///
/// # Layout
///
/// ```text
/// [source_bytes..., 0x00, padding_zeros...]
///  ^                ^     ^
///  0                |     rounded up to 64-byte boundary
///              source_len (sentinel)
/// ```
///
/// The sentinel byte at `source_len` is always `0x00`. All subsequent bytes
/// (cache-line padding) are also `0x00`, ensuring safe reads for `peek()`
/// and `peek2()` near the end of the buffer.
#[derive(Clone, Debug)]
pub struct SourceBuffer {
    /// Owned buffer: `[source_bytes..., 0x00 sentinel, 0x00 padding...]`.
    buf: Vec<u8>,
    /// Length of the actual source content (excludes sentinel and padding).
    source_len: u32,
    /// Encoding issues detected during construction.
    encoding_issues: Vec<EncodingIssue>,
}

/// Encoding issue detected during source buffer construction.
///
/// Carries the kind, byte position, and byte length of the problematic
/// sequence. The integration layer converts these to `LexError` diagnostics
/// using `Span::new(pos, pos + len)` — no need to hard-code per-kind lengths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodingIssue {
    /// What kind of encoding issue was detected.
    pub kind: EncodingIssueKind,
    /// Byte position in the source where the issue was found.
    pub pos: u32,
    /// Byte length of the problematic sequence.
    pub len: u32,
}

/// Kind of encoding issue detected in source buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncodingIssueKind {
    /// UTF-8 BOM (`0xEF 0xBB 0xBF`) at start. Forbidden per Ori spec.
    Utf8Bom,
    /// UTF-16 Little-Endian BOM (`0xFF 0xFE`) at start. Wrong encoding.
    Utf16LeBom,
    /// UTF-16 Big-Endian BOM (`0xFE 0xFF`) at start. Wrong encoding.
    Utf16BeBom,
    /// Null byte (U+0000) in source content. Forbidden per grammar.
    InteriorNull,
}

impl SourceBuffer {
    /// Create a new sentinel-terminated buffer from source code.
    ///
    /// Copies the source bytes into a cache-line-aligned buffer with a
    /// `0x00` sentinel byte appended. Scans for encoding issues (BOMs,
    /// interior null bytes) and records them.
    ///
    /// # File Size
    ///
    /// Source files larger than `u32::MAX` bytes (~4 GiB) are accepted but
    /// the `source_len` field saturates at `u32::MAX`. The compiler
    /// (`ori_lexer`) detects and reports oversized files upstream.
    pub fn new(source: &str) -> Self {
        let source_bytes = source.as_bytes();
        let source_len = source_bytes.len();

        // Round up to next 64-byte boundary (minimum: source + 1 sentinel byte).
        let padded_len = (source_len + 1 + CACHE_LINE - 1) & !(CACHE_LINE - 1);

        // Allocate zero-filled buffer, then copy source bytes.
        // The sentinel (buf[source_len]) and padding are already 0x00.
        let mut buf = vec![0u8; padded_len];
        buf[..source_len].copy_from_slice(source_bytes);

        // Prefetch first cache lines for scanner warmup.
        prefetch_buffer(&buf);

        // Detect encoding issues (BOMs, interior nulls).
        let mut encoding_issues = Vec::new();
        detect_encoding_issues(source_bytes, &mut encoding_issues);

        // Saturate source_len to u32::MAX for files > 4 GiB.
        let source_len_u32 = u32::try_from(source_len).unwrap_or(u32::MAX);

        Self {
            buf,
            source_len: source_len_u32,
            encoding_issues,
        }
    }

    /// Returns the source bytes (without sentinel or padding).
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.source_len as usize]
    }

    /// Returns the full buffer including sentinel and cache-line padding.
    ///
    /// The byte at index [`len()`](Self::len) is the sentinel (`0x00`).
    /// Subsequent bytes are zero-filled padding up to the next 64-byte boundary.
    pub fn as_sentinel_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Create a [`Cursor`] positioned at byte 0.
    pub fn cursor(&self) -> Cursor<'_> {
        Cursor::new(&self.buf, self.source_len)
    }

    /// Length of the source content in bytes (excludes sentinel and padding).
    pub fn len(&self) -> u32 {
        self.source_len
    }

    /// Returns `true` if the source content is empty.
    pub fn is_empty(&self) -> bool {
        self.source_len == 0
    }

    /// Encoding issues detected during construction.
    ///
    /// The integration layer (`ori_lexer`) converts these into diagnostic
    /// errors with proper spans and messages.
    pub fn encoding_issues(&self) -> &[EncodingIssue] {
        &self.encoding_issues
    }
}

/// Size assertion: `SourceBuffer` should be ~56 bytes on 64-bit platforms.
/// Vec<u8> = 24, u32 = 4, Vec<EncodingIssue> = 24, + 4 padding = 56.
const _: () = assert!(std::mem::size_of::<SourceBuffer>() <= 64);

/// Detect BOM and interior null byte issues in source bytes.
fn detect_encoding_issues(source: &[u8], issues: &mut Vec<EncodingIssue>) {
    detect_bom(source, issues);
    detect_interior_nulls(source, issues);
}

/// Detect byte order marks at the start of the source.
fn detect_bom(source: &[u8], issues: &mut Vec<EncodingIssue>) {
    if source.len() >= 3 && source[0] == 0xEF && source[1] == 0xBB && source[2] == 0xBF {
        issues.push(EncodingIssue {
            kind: EncodingIssueKind::Utf8Bom,
            pos: 0,
            len: 3,
        });
    } else if source.len() >= 2 {
        if source[0] == 0xFF && source[1] == 0xFE {
            issues.push(EncodingIssue {
                kind: EncodingIssueKind::Utf16LeBom,
                pos: 0,
                len: 2,
            });
        } else if source[0] == 0xFE && source[1] == 0xFF {
            issues.push(EncodingIssue {
                kind: EncodingIssueKind::Utf16BeBom,
                pos: 0,
                len: 2,
            });
        }
    }
}

/// Detect null bytes (U+0000) within the source content.
///
/// Uses `memchr` for SIMD-accelerated null byte search instead of
/// byte-at-a-time iteration.
fn detect_interior_nulls(source: &[u8], issues: &mut Vec<EncodingIssue>) {
    let mut offset = 0;
    while let Some(pos) = memchr::memchr(0, &source[offset..]) {
        let absolute = offset + pos;
        if let Ok(p) = u32::try_from(absolute) {
            issues.push(EncodingIssue {
                kind: EncodingIssueKind::InteriorNull,
                pos: p,
                len: 1,
            });
        }
        offset = absolute + 1;
    }
}

/// Hint the CPU to prefetch the first 4 cache lines (256 bytes) of the buffer.
///
/// Warms up L1 cache for the scanner's initial reads. On platforms without
/// prefetch support, this is a no-op.
#[allow(unsafe_code)]
fn prefetch_buffer(buf: &[u8]) {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: `_mm_prefetch` is a hint instruction. The CPU silently ignores
        // prefetch requests for invalid or unmapped addresses. All addresses here
        // point within the allocated Vec buffer.
        unsafe {
            use std::arch::x86_64::_mm_prefetch;
            let p = buf.as_ptr().cast::<i8>();
            _mm_prefetch::<3>(p); // _MM_HINT_T0: prefetch into all cache levels
            if buf.len() >= 64 {
                _mm_prefetch::<3>(p.add(64));
            }
            if buf.len() >= 128 {
                _mm_prefetch::<3>(p.add(128));
            }
            if buf.len() >= 192 {
                _mm_prefetch::<3>(p.add(192));
            }
        }
    }

    #[cfg(not(target_arch = "x86_64"))]
    let _ = buf;
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Construction ===

    #[test]
    fn empty_source() {
        let buf = SourceBuffer::new("");
        assert_eq!(buf.len(), 0);
        assert!(buf.is_empty());
        assert!(buf.as_bytes().is_empty());
        assert!(buf.encoding_issues().is_empty());
        // Sentinel present at index 0
        assert_eq!(buf.as_sentinel_bytes()[0], 0);
    }

    #[test]
    fn ascii_source() {
        let buf = SourceBuffer::new("hello");
        assert_eq!(buf.len(), 5);
        assert!(!buf.is_empty());
        assert_eq!(buf.as_bytes(), b"hello");
        assert!(buf.encoding_issues().is_empty());
        // Sentinel after source bytes
        assert_eq!(buf.as_sentinel_bytes()[5], 0);
    }

    #[test]
    fn utf8_multibyte_source() {
        let source = "hello \u{1F600} world"; // emoji (4 bytes)
        let buf = SourceBuffer::new(source);
        assert_eq!(buf.len() as usize, source.len());
        assert_eq!(buf.as_bytes(), source.as_bytes());
        assert!(buf.encoding_issues().is_empty());
    }

    // === Cache-Line Alignment ===

    #[test]
    fn buffer_aligned_to_cache_line() {
        // Buffer size should be a multiple of 64
        for len in [0, 1, 10, 63, 64, 65, 127, 128, 1000] {
            let source: String = "x".repeat(len);
            let buf = SourceBuffer::new(&source);
            assert_eq!(
                buf.as_sentinel_bytes().len() % CACHE_LINE,
                0,
                "buffer length {} is not cache-line aligned for source length {}",
                buf.as_sentinel_bytes().len(),
                len
            );
        }
    }

    #[test]
    fn sentinel_and_padding_are_zero() {
        let buf = SourceBuffer::new("abc");
        let sentinel_bytes = buf.as_sentinel_bytes();
        // Everything after source content should be zero
        for &b in &sentinel_bytes[3..] {
            assert_eq!(b, 0, "non-zero byte in sentinel/padding region");
        }
    }

    // === BOM Detection ===

    #[test]
    fn detects_utf8_bom() {
        // UTF-8 BOM: 0xEF 0xBB 0xBF
        let source = std::str::from_utf8(&[0xEF, 0xBB, 0xBF, b'h', b'i']).unwrap_or("\u{FEFF}hi");
        let buf = SourceBuffer::new(source);
        assert_eq!(buf.encoding_issues().len(), 1);
        assert_eq!(buf.encoding_issues()[0].kind, EncodingIssueKind::Utf8Bom);
        assert_eq!(buf.encoding_issues()[0].pos, 0);
        assert_eq!(buf.encoding_issues()[0].len, 3);
    }

    #[test]
    fn detects_utf8_bom_via_unicode() {
        // The BOM character U+FEFF encoded as UTF-8 is 0xEF 0xBB 0xBF
        let source = "\u{FEFF}hello";
        let buf = SourceBuffer::new(source);
        assert_eq!(buf.encoding_issues().len(), 1);
        assert_eq!(buf.encoding_issues()[0].kind, EncodingIssueKind::Utf8Bom);
        assert_eq!(buf.encoding_issues()[0].len, 3);
    }

    #[test]
    fn no_bom_in_clean_source() {
        let buf = SourceBuffer::new("let x = 42");
        assert!(buf.encoding_issues().is_empty());
    }

    // === Interior Null Detection ===

    #[test]
    fn detects_interior_null() {
        let source = "ab\0cd";
        let buf = SourceBuffer::new(source);
        let nulls: Vec<_> = buf
            .encoding_issues()
            .iter()
            .filter(|i| i.kind == EncodingIssueKind::InteriorNull)
            .collect();
        assert_eq!(nulls.len(), 1);
        assert_eq!(nulls[0].pos, 2);
        assert_eq!(nulls[0].len, 1);
    }

    #[test]
    fn detects_multiple_interior_nulls() {
        let source = "\0ab\0c\0";
        let buf = SourceBuffer::new(source);
        let nulls: Vec<_> = buf
            .encoding_issues()
            .iter()
            .filter(|i| i.kind == EncodingIssueKind::InteriorNull)
            .collect();
        assert_eq!(nulls.len(), 3);
        assert_eq!(nulls[0].pos, 0);
        assert_eq!(nulls[1].pos, 3);
        assert_eq!(nulls[2].pos, 5);
    }

    #[test]
    fn no_false_positive_nulls() {
        // Source without null bytes should have no InteriorNull issues
        let buf = SourceBuffer::new("hello world\nfoo bar");
        let nulls: Vec<_> = buf
            .encoding_issues()
            .iter()
            .filter(|i| i.kind == EncodingIssueKind::InteriorNull)
            .collect();
        assert!(nulls.is_empty());
    }

    // === Multiple Issues ===

    #[test]
    fn bom_and_null_both_detected() {
        let source = "\u{FEFF}ab\0cd";
        let buf = SourceBuffer::new(source);
        assert_eq!(buf.encoding_issues().len(), 2);
        assert_eq!(buf.encoding_issues()[0].kind, EncodingIssueKind::Utf8Bom);
        assert_eq!(
            buf.encoding_issues()[1].kind,
            EncodingIssueKind::InteriorNull
        );
    }

    // === Large Source ===

    #[test]
    fn large_source() {
        let source: String = "x".repeat(100_000);
        let buf = SourceBuffer::new(&source);
        assert_eq!(buf.len(), 100_000);
        assert_eq!(buf.as_bytes().len(), 100_000);
        assert!(buf.encoding_issues().is_empty());
        // Sentinel is correct
        assert_eq!(buf.as_sentinel_bytes()[100_000], 0);
        // Buffer is cache-line aligned
        assert_eq!(buf.as_sentinel_bytes().len() % CACHE_LINE, 0);
    }

    // === Cursor Creation ===

    #[test]
    fn cursor_starts_at_zero() {
        let buf = SourceBuffer::new("hello");
        let cursor = buf.cursor();
        assert_eq!(cursor.pos(), 0);
        assert_eq!(cursor.current(), b'h');
    }

    #[test]
    fn cursor_on_empty_source_is_eof() {
        let buf = SourceBuffer::new("");
        let cursor = buf.cursor();
        assert!(cursor.is_eof());
        assert_eq!(cursor.current(), 0);
    }
}
