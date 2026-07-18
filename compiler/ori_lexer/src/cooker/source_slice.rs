/// Extract a source slice at the given byte offset and length.
///
/// # Panics
///
/// Panics if the raw scanner returns an out-of-bounds range or splits a UTF-8
/// codepoint.
#[inline]
pub(super) fn slice_source(source: &str, offset: u32, len: u32) -> &str {
    let start = offset as usize;
    assert!(
        start <= source.len(),
        "token start {start} exceeds source length {}",
        source.len()
    );
    let token_len = len as usize;
    assert!(
        token_len <= source.len() - start,
        "token length {token_len} at {start} exceeds source length {}",
        source.len()
    );
    let end = start + token_len;
    assert!(
        source.is_char_boundary(start) && source.is_char_boundary(end),
        "token range {start}..{end} splits a UTF-8 codepoint"
    );
    &source[start..end]
}
