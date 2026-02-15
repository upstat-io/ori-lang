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
