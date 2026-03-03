//! Tests for string slice operations (substring, trim, split).

use crate::rc::{ori_rc_count, ori_rc_dec, ori_rc_free};
use crate::slice_encoding::{is_slice_cap, slice_byte_offset, slice_original_data};
use crate::string::OriStr;

use super::ori_str_substring;
use super::ori_str_trim;

// ── ori_str_substring ─────────────────────────────────────────────────

#[test]
fn substring_of_heap_string_produces_slice() {
    let _g = crate::test_helpers::lock_rc();
    // Create a heap string > 23 bytes
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    // Substring that's also > 23 bytes → must produce a slice
    let sub = ori_str_substring(&source, 4, 34); // "quick brown fox jumps over the"
    assert!(
        !sub.is_sso(),
        "long substring of heap string should be heap"
    );

    let sub_heap = unsafe { &sub.heap };
    assert!(
        is_slice_cap(sub_heap.cap),
        "substring should produce a seamless slice"
    );
    assert_eq!(sub_heap.len, 30); // "quick brown fox jumps over the" = 30 bytes
    assert_eq!(slice_byte_offset(sub_heap.cap), 4); // offset from original data

    // Data pointer should point into the original buffer
    assert_eq!(sub_heap.data, unsafe { heap_data.add(4) });

    // RC should be 2 (original + slice)
    assert_eq!(ori_rc_count(heap_data), 2);

    // Content should be correct
    let sub_bytes = sub.as_bytes();
    assert_eq!(sub_bytes, b"quick brown fox jumps over the");

    // Clean up: dec slice reference, then free original
    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_of_heap_short_result_uses_sso() {
    let _g = crate::test_helpers::lock_rc();
    // Create a heap string > 23 bytes
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    // Take a short substring (<= 23 bytes) → should use SSO, not slice
    let sub = ori_str_substring(&source, 4, 9); // "quick" = 5 bytes
    assert!(sub.is_sso(), "short substring should use SSO, not slice");
    assert_eq!(sub.as_bytes(), b"quick");

    // RC unchanged: SSO doesn't reference the original buffer
    assert_eq!(ori_rc_count(heap_data), 1);

    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_of_sso_string_copies() {
    // Create an SSO string (<= 23 bytes)
    let source = OriStr::from_sso(b"hello world");
    assert!(source.is_sso());

    let sub = ori_str_substring(&source, 6, 11); // "world"
    assert!(sub.is_sso(), "substring of SSO should produce SSO");
    assert_eq!(sub.as_bytes(), b"world");
}

#[test]
fn substring_empty_range_returns_empty() {
    let _g = crate::test_helpers::lock_rc();
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    // start == end → empty
    let sub = ori_str_substring(&source, 5, 5);
    assert!(sub.is_empty());
    assert_eq!(sub.len(), 0);

    // RC unchanged
    assert_eq!(ori_rc_count(heap_data), 1);

    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_full_range_of_heap_produces_slice() {
    let _g = crate::test_helpers::lock_rc();
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    // Full range → slice with offset 0
    let sub = ori_str_substring(&source, 0, 43);
    assert!(!sub.is_sso());

    let sub_heap = unsafe { &sub.heap };
    assert!(is_slice_cap(sub_heap.cap));
    assert_eq!(slice_byte_offset(sub_heap.cap), 0);
    assert_eq!(sub_heap.len, 43);
    assert_eq!(sub_heap.data, heap_data);
    assert_eq!(ori_rc_count(heap_data), 2);

    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_clamping() {
    let source = OriStr::from_sso(b"hello");

    // Negative start clamped to 0
    let sub = ori_str_substring(&source, -5, 3);
    assert_eq!(sub.as_bytes(), b"hel");

    // End beyond length clamped to len
    let sub = ori_str_substring(&source, 2, 100);
    assert_eq!(sub.as_bytes(), b"llo");

    // Start beyond length → empty
    let sub = ori_str_substring(&source, 100, 200);
    assert!(sub.is_empty());
}

#[test]
fn substring_null_returns_empty() {
    let sub = ori_str_substring(std::ptr::null(), 0, 5);
    assert!(sub.is_empty());
}

#[test]
fn substring_of_slice_accumulates_offsets() {
    let _g = crate::test_helpers::lock_rc();
    // Create a heap string and take a slice, then slice the slice
    let source = OriStr::from_heap(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ-extra-padding-data");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    // First slice: offset 4, length 38 → "EFGHIJKLMNOPQRSTUVWXYZ-extra-padding-d"
    let slice1 = ori_str_substring(&source, 4, 42);
    assert_eq!(ori_rc_count(heap_data), 2);
    let s1_heap = unsafe { &slice1.heap };
    assert!(is_slice_cap(s1_heap.cap));
    assert_eq!(slice_byte_offset(s1_heap.cap), 4);

    // Second slice of first slice: offset 2 within slice1 → total offset 6
    // Take 28 bytes: "GHIJKLMNOPQRSTUVWXYZ-extra-p"
    let slice2 = ori_str_substring(&slice1, 2, 30);
    assert_eq!(ori_rc_count(heap_data), 3);
    let s2_heap = unsafe { &slice2.heap };
    assert!(is_slice_cap(s2_heap.cap));
    assert_eq!(slice_byte_offset(s2_heap.cap), 6); // 4 + 2

    // Data should point into original buffer
    assert_eq!(s2_heap.data, unsafe { heap_data.add(6) });
    assert_eq!(slice_original_data(s2_heap.data, s2_heap.cap), heap_data);

    // Content check
    assert_eq!(slice2.as_bytes(), b"GHIJKLMNOPQRSTUVWXYZ-extra-p");

    // Clean up
    ori_rc_dec(heap_data, None); // slice1
    ori_rc_dec(heap_data, None); // slice2
    ori_rc_free(heap_data, 45, 1);
}

// ── ori_str_trim ──────────────────────────────────────────────────────

#[test]
fn trim_heap_string_produces_slice() {
    let _g = crate::test_helpers::lock_rc();
    // Heap string with leading/trailing whitespace, trimmed result > 23 bytes
    let source = OriStr::from_heap(b"   The quick brown fox jumps over the lazy dog   ");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let trimmed = ori_str_trim(&source);
    assert!(!trimmed.is_sso());

    let t_heap = unsafe { &trimmed.heap };
    assert!(
        is_slice_cap(t_heap.cap),
        "trim of heap string should produce a slice"
    );
    assert_eq!(t_heap.len, 43); // "The quick brown fox jumps over the lazy dog"
    assert_eq!(slice_byte_offset(t_heap.cap), 3); // 3 spaces skipped

    assert_eq!(ori_rc_count(heap_data), 2);
    assert_eq!(
        trimmed.as_bytes(),
        b"The quick brown fox jumps over the lazy dog"
    );

    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 49, 1);
}

#[test]
fn trim_sso_string_copies() {
    let source = OriStr::from_sso(b"  hello  ");
    assert!(source.is_sso());

    let trimmed = ori_str_trim(&source);
    assert!(trimmed.is_sso(), "trim of SSO should produce SSO");
    assert_eq!(trimmed.as_bytes(), b"hello");
}

#[test]
fn trim_all_whitespace_returns_empty() {
    let source = OriStr::from_sso(b"   \t\n  ");
    let trimmed = ori_str_trim(&source);
    assert!(trimmed.is_empty());
}

#[test]
fn trim_no_whitespace_full_slice() {
    let _g = crate::test_helpers::lock_rc();
    // Heap string with no whitespace → full slice (offset 0)
    let source = OriStr::from_heap(b"The-quick-brown-fox-jumps-over");
    let heap_data = unsafe { source.heap.data };

    let trimmed = ori_str_trim(&source);
    assert!(!trimmed.is_sso());

    let t_heap = unsafe { &trimmed.heap };
    assert!(is_slice_cap(t_heap.cap));
    assert_eq!(slice_byte_offset(t_heap.cap), 0);
    assert_eq!(t_heap.len, 30);
    assert_eq!(ori_rc_count(heap_data), 2);

    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 30, 1);
}

#[test]
fn trim_null_returns_empty() {
    let trimmed = ori_str_trim(std::ptr::null());
    assert!(trimmed.is_empty());
}

#[test]
fn trim_heap_short_result_uses_sso() {
    let _g = crate::test_helpers::lock_rc();
    // Heap string with lots of whitespace, only "hi" remains (2 bytes → SSO)
    let padded = b"                        hi                        ";
    let source = OriStr::from_heap(padded);
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let trimmed = ori_str_trim(&source);
    assert!(
        trimmed.is_sso(),
        "short trim result should use SSO, not slice"
    );
    assert_eq!(trimmed.as_bytes(), b"hi");

    // RC unchanged: SSO doesn't reference original buffer
    assert_eq!(ori_rc_count(heap_data), 1);

    ori_rc_free(heap_data, padded.len(), 1);
}

// ── ori_str_split slices ──────────────────────────────────────────────

#[test]
fn split_heap_string_long_pieces_are_slices() {
    use crate::string::ori_str_split;
    let _g = crate::test_helpers::lock_rc();

    // Create a heap string with two long pieces separated by "|"
    // "ABCDEFGHIJKLMNOPQRSTUVWX|abcdefghijklmnopqrstuvwx" = 49 bytes
    let content = b"ABCDEFGHIJKLMNOPQRSTUVWX|abcdefghijklmnopqrstuvwx";
    let source = OriStr::from_heap(content);
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let sep = b"|";
    let mut out = [0u8; 24];

    ori_str_split(
        heap_data,
        content.len() as i64,
        sep.as_ptr(),
        sep.len() as i64,
        out.as_mut_ptr(),
    );

    // Read list result
    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let list_data = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };
    assert_eq!(len, 2);

    // Read the two OriStr elements
    let elements = list_data.cast::<OriStr>();
    let elem0 = unsafe { elements.read() };
    let elem1 = unsafe { elements.add(1).read() };

    // Both pieces are 24 bytes > 23, so they should be slices
    assert!(
        !elem0.is_sso(),
        "first piece (24 bytes) should be heap/slice"
    );
    assert!(
        !elem1.is_sso(),
        "second piece (24 bytes) should be heap/slice"
    );

    let e0_heap = unsafe { &elem0.heap };
    let e1_heap = unsafe { &elem1.heap };

    assert!(is_slice_cap(e0_heap.cap), "first piece should be a slice");
    assert!(is_slice_cap(e1_heap.cap), "second piece should be a slice");

    assert_eq!(e0_heap.len, 24);
    assert_eq!(e1_heap.len, 24);

    // Verify offsets
    assert_eq!(slice_byte_offset(e0_heap.cap), 0); // starts at beginning
    assert_eq!(slice_byte_offset(e1_heap.cap), 25); // starts after "ABCDEFGHIJKLMNOPQRSTUVWX|"

    // Verify content
    assert_eq!(elem0.as_bytes(), b"ABCDEFGHIJKLMNOPQRSTUVWX");
    assert_eq!(elem1.as_bytes(), b"abcdefghijklmnopqrstuvwx");

    // RC: original + 2 slices = 3
    assert_eq!(ori_rc_count(heap_data), 3);

    // Clean up: dec both slice references, then free original and list
    ori_rc_dec(heap_data, None);
    ori_rc_dec(heap_data, None);
    crate::rc::ori_rc_free(
        list_data,
        2 * std::mem::size_of::<OriStr>(),
        std::mem::align_of::<OriStr>(),
    );
    ori_rc_free(heap_data, content.len(), 1);
}

#[test]
fn split_heap_string_short_pieces_use_sso() {
    use crate::string::ori_str_split;
    let _g = crate::test_helpers::lock_rc();

    // Heap string where pieces are all short (< 24 bytes)
    let content = b"hello world foo bar baz qux quux";
    let source = OriStr::from_heap(content);
    let heap_data = unsafe { source.heap.data };

    let sep = b" ";
    let mut out = [0u8; 24];

    ori_str_split(
        heap_data,
        content.len() as i64,
        sep.as_ptr(),
        sep.len() as i64,
        out.as_mut_ptr(),
    );

    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let list_data = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };
    assert_eq!(len, 7);

    // All pieces are short → SSO, no slice
    let elements = list_data.cast::<OriStr>();
    for i in 0..7 {
        let elem = unsafe { elements.add(i).read() };
        assert!(elem.is_sso(), "short split piece {i} should be SSO");
    }

    // Check first and last
    assert_eq!(unsafe { elements.read() }.as_bytes(), b"hello");
    assert_eq!(unsafe { elements.add(6).read() }.as_bytes(), b"quux");

    // RC unchanged: SSO pieces don't reference original
    assert_eq!(ori_rc_count(heap_data), 1);

    crate::rc::ori_rc_free(
        list_data,
        7 * std::mem::size_of::<OriStr>(),
        std::mem::align_of::<OriStr>(),
    );
    ori_rc_free(heap_data, content.len(), 1);
}
